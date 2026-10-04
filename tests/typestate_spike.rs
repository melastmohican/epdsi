#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Design spike for backlog item `0c` (`.agents/notes/backlog.md`): can a compile-time
//! init-before-use typestate compose with `epdsi`'s existing generic `EpdDriver<BUS, CONTROLLER,
//! PANEL>` and its blocking/async dual API, at an acceptable ergonomic and compile-time cost?
//!
//! This is a prototype, not a change to the shipped API. `TypestateDriver` wraps `EpdDriver` by
//! composition rather than editing `src/driver.rs`, so nothing here affects the real library if
//! the spike's findings say not to proceed. See the bottom of this file for the write-up.

use core::marker::PhantomData;
#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;

use embedded_hal::digital::{Error as DigitalError, ErrorKind, ErrorType, OutputPin};
use epdsi::prelude::*;
use epdsi::traits::{EpdController, EpdPanel};

mod support;
use support::*;

/// A reset pin that always errors, local to this spike (not added to the shared `support`
/// module, which has no non-`Infallible` pin double: every shared mock pin is `Infallible` by
/// type, and the blocking `wait_busy_with_delay`'s 60,000-retry timeout returns `Ok` rather than
/// an error, so neither can actually force `init_sequence` to fail). `hard_reset`'s first call
/// is `rst.set_high()`, so this fails `init` immediately and deterministically in both feature
/// sets, with no busy-wait timing involved.
#[derive(Debug)]
struct AlwaysFailsError;
impl DigitalError for AlwaysFailsError {
    fn kind(&self) -> ErrorKind {
        ErrorKind::Other
    }
}

#[derive(Debug)]
struct FailingResetPin;
impl ErrorType for FailingResetPin {
    type Error = AlwaysFailsError;
}
impl OutputPin for FailingResetPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Err(AlwaysFailsError)
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Err(AlwaysFailsError)
    }
}

/// Marker type: no `init_sequence` has run yet. No methods except `init` exist in this state.
pub struct Uninit;
/// Marker type: `init_sequence` has run. Every normal operation becomes available.
pub struct Init;

/// Wraps an `EpdDriver`, gating every operation except `init` behind a compile-time state
/// parameter. Delegates to the real `EpdDriver` methods rather than reimplementing them, so it
/// stays byte-identical to what's already shipped and tested.
pub struct TypestateDriver<BUS, CONTROLLER, PANEL, STATE> {
    inner: EpdDriver<BUS, CONTROLLER, PANEL>,
    _state: PhantomData<STATE>,
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<BUS, CONTROLLER, PANEL> TypestateDriver<BUS, CONTROLLER, PANEL, Uninit>
where
    CONTROLLER: EpdController<BUS>,
    PANEL: EpdPanel,
{
    pub fn new(inner: EpdDriver<BUS, CONTROLLER, PANEL>) -> Self {
        Self {
            inner,
            _state: PhantomData,
        }
    }

    /// Consumes the `Uninit` driver. On success, returns an `Init` driver with every other
    /// method now available. On failure, hands the caller back their `Uninit` driver (and so
    /// their bus/controller) alongside the error, so a failed `init` doesn't strand the SPI
    /// device and GPIO pins inside a value the caller can no longer use. `epd-spectra`'s own
    /// `Epd::init` drops `self` on the error path instead, losing the wrapped resources.
    #[allow(clippy::type_complexity)]
    pub async fn init<DELAY: DelayNs>(
        mut self,
        delay: &mut DELAY,
    ) -> Result<TypestateDriver<BUS, CONTROLLER, PANEL, Init>, (Self, CONTROLLER::Error)> {
        match self.inner.init(delay).await {
            Ok(()) => Ok(TypestateDriver {
                inner: self.inner,
                _state: PhantomData,
            }),
            Err(e) => Err((self, e)),
        }
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<BUS, CONTROLLER, PANEL> TypestateDriver<BUS, CONTROLLER, PANEL, Init>
where
    CONTROLLER: EpdController<BUS>,
    PANEL: EpdPanel,
{
    pub async fn write_frame(
        &mut self,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), CONTROLLER::Error> {
        self.inner.write_frame(channel, data).await
    }

    pub async fn clear_frame(
        &mut self,
        channel: ColorChannel,
        pattern_byte: u8,
    ) -> Result<(), CONTROLLER::Error> {
        self.inner.clear_frame(channel, pattern_byte).await
    }

    pub async fn refresh<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<(), CONTROLLER::Error> {
        self.inner.refresh(delay).await
    }

    pub async fn sleep<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<(), CONTROLLER::Error> {
        self.inner.sleep(delay).await
    }
}

// --- Spike tests: prove the happy path and the failure-recovery path on three different
// controllers (SSD1681, UC8253, SSD1677), the minimum the backlog item asks for. ---

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1681_typestate_happy_path_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ssd1681Controller::new(GDEM0154Z90::WIDTH, GDEM0154Z90::HEIGHT);
    let uninit = TypestateDriver::new(EpdBuilder::<_, GDEM0154Z90>::new(controller).build(bus));

    // `uninit.write_frame(...)` would not compile here: that method only exists on
    // `TypestateDriver<_, _, _, Init>`. This is the property the spike is for.
    let mut init = uninit.init(&mut delay).await.unwrap_or_else(|_| panic!());

    init.clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("clear_frame failed");
    init.refresh(&mut delay).await.expect("refresh failed");
    init.sleep(&mut delay).await.expect("sleep failed");
}
epd_test!(
    test_ssd1681_typestate_happy_path,
    ssd1681_typestate_happy_path_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_typestate_happy_path_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
        .with_variant(Uc8253Variant::Se0352n14);
    let uninit =
        TypestateDriver::new(EpdBuilder::<_, SE0352N14TNGA0>::new(controller).build(bus));

    let mut init = uninit.init(&mut delay).await.unwrap_or_else(|_| panic!());
    init.clear_frame(ColorChannel::BlackWhite, 0x00)
        .await
        .expect("clear_frame failed");
    init.refresh(&mut delay).await.expect("refresh failed");
}
epd_test!(
    test_uc8253_typestate_happy_path,
    uc8253_typestate_happy_path_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_typestate_happy_path_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let uninit = TypestateDriver::new(EpdBuilder::<_, GDEQ0426T82>::new(controller).build(bus));

    let mut init = uninit.init(&mut delay).await.unwrap_or_else(|_| panic!());
    init.clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("clear_frame failed");
    init.refresh(&mut delay).await.expect("refresh failed");
}
epd_test!(
    test_ssd1677_typestate_happy_path,
    ssd1677_typestate_happy_path_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn failed_init_returns_the_caller_their_driver_back_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, FailingResetPin, FixedPin(true));
    let controller = Ssd1681Controller::new(GDEM0154Z90::WIDTH, GDEM0154Z90::HEIGHT);
    let uninit = TypestateDriver::new(EpdBuilder::<_, GDEM0154Z90>::new(controller).build(bus));

    // `EpdDriver` has no `Debug` impl, so `Result::unwrap_err` (which needs the Ok side to be
    // `Debug`) isn't available here; match manually instead.
    let recovered = match uninit.init(&mut delay).await {
        Ok(_) => panic!("expected init to fail: FailingResetPin always errors"),
        Err((recovered, _err)) => recovered,
    };
    // `recovered` is a live `Uninit` driver again: the bus/controller weren't dropped on the
    // error path. Retrying (and failing the same way, since the pin still always errors) proves
    // the recovered value is a real, usable `TypestateDriver`, not a moved-out husk.
    if recovered.init(&mut delay).await.is_ok() {
        panic!("expected the retry to fail too");
    }
}
epd_test!(
    test_failed_init_returns_the_caller_their_driver_back,
    failed_init_returns_the_caller_their_driver_back_body
);

// --- Findings (2026-10-03) ---
//
// 1. The mechanism composes cleanly with `maybe_async_cfg`: the typestate-gated methods are
//    plain `async fn`s like every other `epdsi` method, so the existing blocking/async dual-API
//    macro handles them with zero special-casing. Both feature sets pass.
//
// 2. It does NOT compose for free with the paged-rendering free functions
//    (`render_paged`/`render_paged_tri_color`/`render_paged_gray4`/`render_paged_gray4_preclear`
//    in `src/graphics/paged.rs`). Each one is typed `driver: &mut EpdDriver<BUS, CONTROLLER,
//    PANEL>` today; adopting this pattern in `EpdDriver` itself (not just a wrapper, as here)
//    would need each signature changed to `&mut EpdDriver<BUS, CONTROLLER, PANEL, Init>`. Four
//    call sites, mechanical, but the original backlog item's file list (`src/driver.rs` ·
//    `src/traits.rs`) missed this; `src/graphics/paged.rs` would need touching too.
//
// 3. `epd-spectra`'s own `Epd::init` drops `self` on the error path, losing the caller's SPI
//    device and GPIO pins on a failed init with no way to retry. Porting that behavior directly
//    would be a real regression from `epdsi`'s current `&mut self` `init`, which leaves the
//    driver alive and retryable. This spike's `init` returns `(Self, Error)` on failure instead,
//    fixing that at the cost of callers needing to destructure a tuple error rather than a plain
//    one. That's a real ergonomic trade worth deciding on explicitly, not inheriting silently.
//
// 4. Migrating this into the real `EpdDriver` would be a breaking change for every call site in
//    every downstream hardware-example repo (`rust-rpico2-discovery`,
//    `rust-rpico2-embassy-examples`, `adafruit-feather-thinkink-discovery`,
//    `xiao-esp32c3-blinky`, `rust-reterminal-e1002-examples`). `init(&mut self)` becoming
//    `init(self)` changes every example's binding from `let mut epd = ...` to needing
//    `let epd = ...; let mut epd = epd.init(...)?;`. Confirmed by grepping all five repos
//    locally, not estimated: every example in those repos currently calls
//    `epd.init(&mut delay)`/`driver.init(&mut delay)` on a `mut` binding
//    created one line earlier.
//
// 5. Compile-time/binary-size cost is not measurable above noise: `PhantomData<STATE>` is
//    zero-sized, and the gated methods are the same `async fn`s that already exist on
//    `EpdDriver`, just relocated to a type-parameterized impl block. No new monomorphization
//    bound was added (STATE is a plain, unconstrained type parameter, not a trait bound), so no
//    additional codegen is produced per panel/controller pairing.
//
// 6. The failure this whole design protects against is rare, and the failure users actually hit
//    is a different one it can't touch. `wait_busy_with_delay` (`src/bus.rs:314-333`) doesn't
//    error on its own 60,000-retry timeout: it just breaks the loop and returns `Ok(())` anyway,
//    so a genuinely stuck or disconnected panel doesn't fail `init()` today. The remaining ways
//    `init_sequence` can return `Err` are a GPIO or SPI transfer actually erroring, rare on real
//    MCU HALs for a correctly wired board. Checked this project's own GitHub issues for real
//    evidence rather than assuming: the one open report
//    (https://github.com/melastmohican/epdsi/issues/7) is a user who got far enough to read the
//    panel's ID but couldn't get it to refresh, not an `init()` error they mishandled. That's
//    the actual shape of failure this crate's users hit: everything appears to succeed while the
//    hardware doesn't cooperate, which no amount of type-system ordering enforcement can catch,
//    because the mistake isn't "called things in the wrong order."
//
// Verdict: don't pursue this, at least not now. The mechanism works, but the cost doesn't clear
// the bar the benefit sets. What it guards against (calling a method before `init`) is close to
// self-diagnosing already: the panel just does nothing on the first run, caught on the bench in
// minutes, not in the field. What it costs is real and confirmed, not estimated: a breaking
// change across all 5 downstream example repos plus `src/graphics/paged.rs` (items 2 and 4), a
// tuple-error-vs-plain-error decision with no clean answer (item 3), and a permanent tax on
// every future controller/method addition to keep it correctly sorted into the right impl block.
// Per item 6, none of that cost buys anything against the failure mode users actually hit, a
// silent hardware problem that looks like success at every type-checked step. The one scenario
// that would flip this (a crate with no shipping examples yet, so no breaking-change cost at
// all) already closed for `epdsi`. Leave this item closed unless something changes that
// materially cheapens the migration cost or raises the real frequency of the ordering mistake
// above what's been observed.

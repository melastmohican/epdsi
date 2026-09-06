#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.

use core::cell::RefCell;
use std::collections::VecDeque;

use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
use epdsi::prelude::*;

mod support;
use support::*;

// ---------------------------------------------------------------------------------------------
// Bit-banged Spi3Bus mock: reconstructs whole bytes from the individual bit-level
// set_as_output/set_high/set_low/set_as_input/is_high calls Spi3Bus::write_byte/read_byte make,
// so tests can assert byte-level command/data sequences instead of raw bit toggling. None of
// these pin/state types are async themselves (`Spi3Bus` needs no `Wait` — see `src/bus3.rs`'s
// module doc); only the test bodies driving `Spi3Bus`'s own methods need `.await`.
// ---------------------------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    /// A byte written with DC in the given state (true = Data/high, false = Command/low).
    Write(bool, u8),
    /// A byte read with DC in the given state at the time of the read.
    Read(bool, u8),
}

impl Event {
    fn cmd(byte: u8) -> Self {
        Event::Write(false, byte)
    }
    fn data(byte: u8) -> Self {
        Event::Write(true, byte)
    }
    fn read(byte: u8) -> Self {
        Event::Read(true, byte)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Output,
    Input,
}

#[derive(Debug)]
struct MockState {
    cs_low: RefCell<bool>,
    dc_high: RefCell<bool>,
    mode: RefCell<Mode>,
    events: RefCell<Vec<Event>>,
    read_queue: RefCell<VecDeque<u8>>,
    write_bit_buf: RefCell<u8>,
    write_bit_count: RefCell<u8>,
    read_byte: RefCell<u8>,
    read_bit_count: RefCell<u8>,
}

impl MockState {
    fn new(canned_reads: &[u8]) -> Self {
        Self {
            cs_low: RefCell::new(false),
            dc_high: RefCell::new(false),
            mode: RefCell::new(Mode::Output),
            events: RefCell::new(Vec::new()),
            read_queue: RefCell::new(canned_reads.iter().copied().collect()),
            write_bit_buf: RefCell::new(0),
            write_bit_count: RefCell::new(0),
            read_byte: RefCell::new(0),
            read_bit_count: RefCell::new(0),
        }
    }
}

struct MockCs<'a>(&'a MockState);
impl DigitalErrorType for MockCs<'_> {
    type Error = core::convert::Infallible;
}
impl OutputPin for MockCs<'_> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        assert!(
            !*self.0.cs_low.borrow(),
            "CS asserted while already selected"
        );
        *self.0.cs_low.borrow_mut() = true;
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        *self.0.cs_low.borrow_mut() = false;
        Ok(())
    }
}

struct MockSck<'a>(#[allow(dead_code)] &'a MockState);
impl DigitalErrorType for MockSck<'_> {
    type Error = core::convert::Infallible;
}
impl OutputPin for MockSck<'_> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct MockDc<'a>(&'a MockState);
impl DigitalErrorType for MockDc<'_> {
    type Error = core::convert::Infallible;
}
impl OutputPin for MockDc<'_> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        *self.0.dc_high.borrow_mut() = false;
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        *self.0.dc_high.borrow_mut() = true;
        Ok(())
    }
}

struct MockData<'a>(&'a MockState);
impl DigitalErrorType for MockData<'_> {
    type Error = core::convert::Infallible;
}
impl DynamicPin for MockData<'_> {
    type Error = core::convert::Infallible;

    fn set_as_output(&mut self) -> Result<(), Self::Error> {
        *self.0.mode.borrow_mut() = Mode::Output;
        *self.0.write_bit_buf.borrow_mut() = 0;
        *self.0.write_bit_count.borrow_mut() = 0;
        Ok(())
    }

    fn set_as_input(&mut self) -> Result<(), Self::Error> {
        *self.0.mode.borrow_mut() = Mode::Input;
        *self.0.read_byte.borrow_mut() = self
            .0
            .read_queue
            .borrow_mut()
            .pop_front()
            .expect("read past end of canned bytes");
        *self.0.read_bit_count.borrow_mut() = 0;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        self.push_write_bit(1);
        Ok(())
    }

    fn set_low(&mut self) -> Result<(), Self::Error> {
        self.push_write_bit(0);
        Ok(())
    }

    fn is_high(&mut self) -> Result<bool, Self::Error> {
        assert_eq!(*self.0.mode.borrow(), Mode::Input);
        let mut count = self.0.read_bit_count.borrow_mut();
        let byte = *self.0.read_byte.borrow();
        let bit = (byte >> (7 - *count)) & 1 != 0;
        *count += 1;
        if *count == 8 {
            let dc = *self.0.dc_high.borrow();
            self.0.events.borrow_mut().push(Event::Read(dc, byte));
        }
        Ok(bit)
    }
}

impl MockData<'_> {
    fn push_write_bit(&self, bit: u8) {
        assert_eq!(*self.0.mode.borrow(), Mode::Output);
        let mut buf = self.0.write_bit_buf.borrow_mut();
        let mut count = self.0.write_bit_count.borrow_mut();
        *buf = (*buf << 1) | bit;
        *count += 1;
        if *count == 8 {
            let dc = *self.0.dc_high.borrow();
            self.0.events.borrow_mut().push(Event::Write(dc, *buf));
            *buf = 0;
            *count = 0;
        }
    }
}

struct MockRst<'a>(#[allow(dead_code)] &'a MockState);
impl DigitalErrorType for MockRst<'_> {
    type Error = core::convert::Infallible;
}
impl OutputPin for MockRst<'_> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

struct MockBusyIdle;
impl DigitalErrorType for MockBusyIdle {
    type Error = core::convert::Infallible;
}
impl InputPin for MockBusyIdle {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(true) // active-low busy: HIGH = idle, so wait_busy returns immediately.
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

fn make_bus(
    state: &MockState,
) -> Spi3Bus<MockCs<'_>, MockSck<'_>, MockData<'_>, MockDc<'_>, MockRst<'_>, MockBusyIdle> {
    Spi3Bus::new(
        MockCs(state),
        MockSck(state),
        MockData(state),
        MockDc(state),
        MockRst(state),
        MockBusyIdle,
    )
}

/// Synthetic OTP payload of `len` bytes: byte 0 is the bank-start marker `0xA5`, remaining bytes
/// equal their own index (`otp[i] == i as u8`), so downstream register-slice assertions are trivial.
fn synthetic_otp(len: usize) -> Vec<u8> {
    (0..len)
        .map(|i| if i == 0 { 0xA5 } else { i as u8 })
        .collect()
}

fn slice(otp: &[u8], start: usize, end: usize) -> Vec<u8> {
    otp[start..end].to_vec()
}

#[test]
fn test_pervasive_bwry_panel_dimensions() {
    assert_eq!(E2154QS0F1::WIDTH, 152);
    assert_eq!(E2154QS0F1::HEIGHT, 152);
    assert_eq!(EPD_154_QS_0F::WIDTH, 152);
    assert_eq!(E2417QS0A3::WIDTH, 400);
    assert_eq!(E2417QS0A3::HEIGHT, 300);
    assert_eq!(EPD_417_QS_0A::WIDTH, 400);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn spi3bus_write_cmd_and_data_byte_framing_body() {
    let state = MockState::new(&[]);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;

    bus.write_cmd(&mut delay, 0x70).await.unwrap();
    bus.write_data(&mut delay, 0xA5).await.unwrap();

    assert_eq!(
        state.events.borrow().clone(),
        vec![Event::cmd(0x70), Event::data(0xA5)]
    );
    // CS must be deasserted (unselected) after each byte.
    assert!(!*state.cs_low.borrow());
}
epd_test!(
    test_spi3bus_write_cmd_and_data_byte_framing,
    spi3bus_write_cmd_and_data_byte_framing_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn spi3bus_read_bytes_body() {
    let state = MockState::new(&[0x12, 0x34]);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;

    let a = bus.read_data_byte(&mut delay).await.unwrap();
    let b = bus.read_byte_no_dc(&mut delay).await.unwrap();

    assert_eq!(a, 0x12);
    assert_eq!(b, 0x34);
    assert_eq!(
        state.events.borrow().clone(),
        vec![Event::read(0x12), Event::read(0x34)]
    );
}
epd_test!(test_spi3bus_read_bytes, spi3bus_read_bytes_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_chip_id_mismatch_body() {
    let state = MockState::new(&[0x00, 0x01]);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT);

    let err = controller.read_otp(&mut bus, &mut delay).await.unwrap_err();
    assert_eq!(err, PervasiveBwryOtpError::UnexpectedChipId(0x0001));
}
epd_test!(
    test_pervasive_bwry_chip_id_mismatch,
    pervasive_bwry_chip_id_mismatch_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_chip_id_normalization_body() {
    // 0x8302 raw response must be normalized to 0x0302 before comparison. Checked against
    // DriverA (expects 0x0605) so the mismatch still fires — DriverF's expected id is itself
    // 0x0302, so this same raw response is exercised as part of DriverF's happy-path test below.
    let state = MockState::new(&[0x83, 0x02]);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);

    let err = controller.read_otp(&mut bus, &mut delay).await.unwrap_err();
    assert_eq!(err, PervasiveBwryOtpError::UnexpectedChipId(0x0302));
}
epd_test!(
    test_pervasive_bwry_chip_id_normalization,
    pervasive_bwry_chip_id_normalization_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_driverf_read_otp_sequence_body() {
    let otp = synthetic_otp(48);
    let mut canned = vec![0x03, 0x02]; // chip ID (raw 0x0302, matches DriverF directly)
    canned.push(0xAA); // dummy byte
    canned.extend_from_slice(&otp); // marker (otp[0]=0xA5) + otp[1..48]

    let state = MockState::new(&canned);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverF);

    controller.read_otp(&mut bus, &mut delay).await.unwrap();
    let events = state.events.borrow().clone();

    let mut expected = vec![
        Event::cmd(0x70),
        Event::read(0x03),
        Event::read(0x02),
        Event::cmd(0xa4),
        Event::data(0x15),
        Event::data(0x00),
        Event::data(0x01),
        Event::cmd(0xa1),
        Event::read(0xAA),
        Event::read(0xA5),
    ];
    for &b in &otp[1..48] {
        expected.push(Event::Read(true, b));
    }
    assert_eq!(events, expected);
}
epd_test!(
    test_pervasive_bwry_driverf_read_otp_sequence,
    pervasive_bwry_driverf_read_otp_sequence_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_driverf_invalid_otp_marker_body() {
    // Marker byte comes back wrong; DriverF has no retry/fallback and fails immediately.
    let canned = vec![0x03, 0x02, 0xAA, 0x00];

    let state = MockState::new(&canned);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverF);

    let err = controller.read_otp(&mut bus, &mut delay).await.unwrap_err();
    assert_eq!(err, PervasiveBwryOtpError::InvalidOtpMarker);
}
epd_test!(
    test_pervasive_bwry_driverf_invalid_otp_marker,
    pervasive_bwry_driverf_invalid_otp_marker_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_drivera_read_otp_sequence_body() {
    let otp = synthetic_otp(112);
    let mut canned = vec![0x06, 0x05]; // chip ID
    canned.push(0xAA); // dummy
    canned.extend_from_slice(&otp); // marker + otp[1..112], found on first attempt

    let state = MockState::new(&canned);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);

    controller.read_otp(&mut bus, &mut delay).await.unwrap();
    let events = state.events.borrow().clone();

    let mut expected = vec![
        Event::cmd(0x70),
        Event::read(0x06),
        Event::read(0x05),
        Event::cmd(0xa2),
        Event::data(0x00),
        Event::data(0x15),
        Event::data(0x00),
        Event::cmd(0xa0),
        Event::cmd(0x92),
        Event::read(0xAA),
        Event::read(0xA5),
    ];
    for &b in &otp[1..112] {
        expected.push(Event::Read(true, b));
    }
    assert_eq!(events, expected);
}
epd_test!(
    test_pervasive_bwry_drivera_read_otp_sequence,
    pervasive_bwry_drivera_read_otp_sequence_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_drivera_bank2_fallback_body() {
    let otp = synthetic_otp(112);
    let mut canned = vec![0x06, 0x05]; // chip ID
    canned.push(0xAA); // dummy
    canned.push(0x00); // bad marker at bank 1
    canned.extend(std::iter::repeat(0xFF).take(0x70 - 1)); // discarded bank-2 seek bytes
    canned.push(0xA5); // good marker at bank 2
    canned.extend_from_slice(&otp[1..112]);

    let state = MockState::new(&canned);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);

    controller.read_otp(&mut bus, &mut delay).await.unwrap();
}
epd_test!(
    test_pervasive_bwry_drivera_bank2_fallback,
    pervasive_bwry_drivera_bank2_fallback_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_drivera_invalid_otp_marker_body() {
    let mut canned = vec![0x06, 0x05, 0xAA, 0x00];
    canned.extend(std::iter::repeat(0xFF).take(0x70 - 1));
    canned.push(0x00); // still-bad marker at bank 2

    let state = MockState::new(&canned);
    let mut bus = make_bus(&state);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);

    let err = controller.read_otp(&mut bus, &mut delay).await.unwrap_err();
    assert_eq!(err, PervasiveBwryOtpError::InvalidOtpMarker);
}
epd_test!(
    test_pervasive_bwry_drivera_invalid_otp_marker,
    pervasive_bwry_drivera_invalid_otp_marker_body
);

// ---------------------------------------------------------------------------------------------
// Normal 4-wire SpiBusWrapper-based tests for the EpdController impl (init_sequence, write_frame,
// trigger_refresh, sleep) — these are unaffected by the bit-banged OTP rewrite above.
// ---------------------------------------------------------------------------------------------

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_init_sequence_uses_otp_data_body() {
    // First, populate otp_data via a mocked bit-banged read_otp (DriverF, happy path).
    let otp = synthetic_otp(48);
    let mut canned = vec![0x03, 0x02, 0xAA];
    canned.extend_from_slice(&otp);
    let bitbang_state = MockState::new(&canned);
    let mut bitbang_bus = make_bus(&bitbang_state);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT)
        .with_temperature(25)
        .with_variant(PervasiveBwryVariant::DriverF);
    controller
        .read_otp(&mut bitbang_bus, &mut delay)
        .await
        .unwrap();

    // Then run init_sequence over the normal 4-wire SpiBusWrapper and check it uses the
    // OTP-derived register data (not the raw bit-bang bus).
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0xe0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xe6), // BWRY temperature register (not 0xE5)
            SpiRecord::Data(vec![25]),
            SpiRecord::Command(0xa5),
            SpiRecord::Command(0x01),
            SpiRecord::Data(slice(&otp, 16, 17)),
            SpiRecord::Command(0x00),
            SpiRecord::Data(slice(&otp, 17, 19)),
            SpiRecord::Command(0x03),
            SpiRecord::Data(slice(&otp, 30, 33)),
            SpiRecord::Command(0x06),
            SpiRecord::Data(slice(&otp, 23, 30)),
            SpiRecord::Command(0x50),
            SpiRecord::Data(slice(&otp, 39, 40)),
            SpiRecord::Command(0x60),
            SpiRecord::Data(slice(&otp, 40, 42)),
            SpiRecord::Command(0x61),
            SpiRecord::Data(slice(&otp, 19, 23)),
            SpiRecord::Command(0xe7),
            SpiRecord::Data(slice(&otp, 33, 34)),
            SpiRecord::Command(0xe3),
            SpiRecord::Data(slice(&otp, 42, 43)),
            SpiRecord::Command(0x4d),
            SpiRecord::Data(slice(&otp, 43, 44)),
            SpiRecord::Command(0xb4),
            SpiRecord::Data(slice(&otp, 44, 45)),
            SpiRecord::Command(0xb5),
            SpiRecord::Data(slice(&otp, 45, 46)),
            SpiRecord::Command(0xe9),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x30),
            SpiRecord::Data(vec![0x08]),
        ]
    );
    assert!(!records.contains(&SpiRecord::Command(0x04)));
}
epd_test!(
    test_pervasive_bwry_init_sequence_uses_otp_data,
    pervasive_bwry_init_sequence_uses_otp_data_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_drivera_init_sequence_powers_on_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();
    assert!(records.contains(&SpiRecord::Command(0x04)));
}
epd_test!(
    test_pervasive_bwry_drivera_init_sequence_powers_on,
    pervasive_bwry_drivera_init_sequence_powers_on_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_write_frame_and_pattern_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xAA, 0xBB]),]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0x11, 4)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x11; 4])]
    );
}
epd_test!(
    test_pervasive_bwry_write_frame_and_pattern,
    pervasive_bwry_write_frame_and_pattern_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_bwry_trigger_refresh_and_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverF);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Data(vec![0x00]),
        ]
    );
    bus_backend.records.borrow_mut().clear();
    controller.sleep(&mut bus, &mut delay).await.unwrap();
    // DriverF has no additional shutdown command beyond POWER_OFF.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x02), SpiRecord::Data(vec![0x00])]
    );

    bus_backend.records.borrow_mut().clear();
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x12), SpiRecord::Data(vec![0x00])]
    );
    bus_backend.records.borrow_mut().clear();
    controller.sleep(&mut bus, &mut delay).await.unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x02),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x00, 0x00]),
        ]
    );
}
epd_test!(
    test_pervasive_bwry_trigger_refresh_and_sleep,
    pervasive_bwry_trigger_refresh_and_sleep_body
);

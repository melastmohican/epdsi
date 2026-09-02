#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only.

use core::cell::RefCell;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
use embedded_hal::spi::{ErrorKind, ErrorType as SpiErrorType, Operation, SpiDevice};
use epdsi::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SpiRecord {
    Command(u8),
    Data(Vec<u8>),
}

#[derive(Debug)]
struct RecordingSpiBus {
    records: RefCell<Vec<SpiRecord>>,
    dc_state: RefCell<bool>, // false = Low (Command), true = High (Data)
}

impl RecordingSpiBus {
    fn new() -> Self {
        Self {
            records: RefCell::new(Vec::new()),
            dc_state: RefCell::new(false),
        }
    }
}

impl SpiErrorType for &RecordingSpiBus {
    type Error = ErrorKind;
}

impl SpiDevice for &RecordingSpiBus {
    fn transaction(&mut self, _operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        let is_data = *self.dc_state.borrow();
        if is_data {
            self.records
                .borrow_mut()
                .push(SpiRecord::Data(buf.to_vec()));
        } else {
            for &byte in buf {
                self.records.borrow_mut().push(SpiRecord::Command(byte));
            }
        }
        Ok(())
    }
}

struct TestDc<'a>(&'a RecordingSpiBus);

impl DigitalErrorType for TestDc<'_> {
    type Error = core::convert::Infallible;
}

impl OutputPin for TestDc<'_> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        *self.0.dc_state.borrow_mut() = false;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        *self.0.dc_state.borrow_mut() = true;
        Ok(())
    }
}

#[derive(Debug)]
struct DummyPin;

impl DigitalErrorType for DummyPin {
    type Error = core::convert::Infallible;
}

impl OutputPin for DummyPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl InputPin for DummyPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

struct DummyDelay;

impl DelayNs for DummyDelay {
    fn delay_ns(&mut self, _ns: u32) {}
    fn delay_us(&mut self, _us: u32) {}
    fn delay_ms(&mut self, _ms: u32) {}
}

#[derive(Debug)]
struct MockSpi;

impl SpiErrorType for MockSpi {
    type Error = ErrorKind;
}

impl SpiDevice for MockSpi {
    fn transaction(&mut self, _operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write(&mut self, _buf: &[u8]) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug)]
struct MockOutputPin;

impl DigitalErrorType for MockOutputPin {
    type Error = core::convert::Infallible;
}

impl OutputPin for MockOutputPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug)]
struct MockInputPin {
    is_high: bool,
}

impl DigitalErrorType for MockInputPin {
    type Error = core::convert::Infallible;
}

impl InputPin for MockInputPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.is_high)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.is_high)
    }
}

#[test]
fn test_uc8253_gdey037t03_panel_dimensions() {
    assert_eq!(GDEY037T03::WIDTH, 240);
    assert_eq!(GDEY037T03::HEIGHT, 416);
    assert_eq!(GxEPD2_370_GDEY037T03::WIDTH, 240);
}

#[test]
fn test_uc8253_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1E, 0x0D]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
        ]
    );
}

#[test]
fn test_uc8253_set_window_byte_alignment() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);

    // set_window only records the area; the UC8253 needs the window re-opened around every
    // operation, so no commands are emitted here.
    // x_start=10 (rounds down to 8 via &0xFFF8), x_end=20 (rounds up to 23 via |0x0007)
    controller.set_window(&mut bus, 10, 5, 20, 15).unwrap();
    assert!(bus_backend.records.borrow().is_empty());

    // The recorded area is emitted around the next RAM write, and closed again afterwards.
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x91), // PARTIAL_IN
            SpiRecord::Command(0x90), // PARTIAL_WINDOW
            SpiRecord::Data(vec![8, 23, 0x00, 0x05, 0x00, 0x0F, 0x01]),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0xAA]),
            SpiRecord::Command(0x92), // PARTIAL_OUT
        ]
    );

    // clear_window returns to full-frame addressing: no window commands at all.
    bus_backend.records.borrow_mut().clear();
    controller.clear_window();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xBB])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x13), SpiRecord::Data(vec![0xBB])]
    );
}

#[test]
fn test_uc8253_write_frame_channel_routing() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x13), SpiRecord::Data(vec![0xAA, 0xBB]),]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xCC])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xCC])]
    );
}

#[test]
fn test_uc8253_trigger_refresh_all_modes() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // Full (default), no window set: no PARTIAL_OUT, CDI=0x97, no CCSET/TSSET.
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x97]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
        ]
    );

    // FastFull: CCSET/TSSET(0x5A) before CDI=0x97, panel-setting reset after PowerOff.
    bus_backend.records.borrow_mut().clear();
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::FastFull);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0xE0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xE5),
            SpiRecord::Data(vec![0x5A]),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x97]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1E, 0x0D]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
        ]
    );

    // Partial with a window set: the refresh gets its own PARTIAL_IN/PARTIAL_WINDOW session,
    // closed by PARTIAL_OUT only after DISPLAY_REFRESH completes.
    bus_backend.records.borrow_mut().clear();
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::Partial);
    controller.set_window(&mut bus, 0, 0, 239, 415).unwrap();
    bus_backend.records.borrow_mut().clear();
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x91), // PARTIAL_IN, re-opened for the refresh
            SpiRecord::Command(0x90),
            SpiRecord::Data(vec![0, 239, 0x00, 0x00, 0x01, 0x9F, 0x01]),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0xD7]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x92), // PARTIAL_OUT, after the refresh
        ]
    );

    // FastPartial: CCSET/TSSET(0x6E) before CDI=0xD7.
    bus_backend.records.borrow_mut().clear();
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::FastPartial);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0xE0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xE5),
            SpiRecord::Data(vec![0x6E]),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0xD7]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1E, 0x0D]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
        ]
    );
}

#[test]
fn test_uc8253_sleep() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xA5]),
        ]
    );
}

#[test]
fn test_uc8253_busy_low_polarity_smoke() {
    let spi = MockSpi;
    let dc = MockOutputPin;
    let rst = MockOutputPin;
    let busy = MockInputPin { is_high: true }; // Active-low BUSY: high means idle
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(spi, dc, rst, busy);
    let controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEY037T03>::new(controller).build(bus);

    driver.init(&mut delay).expect("UC8253 init failed");
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .expect("UC8253 clear frame failed");
    driver.refresh(&mut delay).expect("UC8253 refresh failed");
    driver.sleep(&mut delay).expect("UC8253 sleep failed");
}

#[test]
fn test_uc8253_se0352n14_panel_dimensions() {
    // 240 x 360 is the raster, not Waveshare's advertised 360 x 240 viewing orientation:
    // 30 bytes per line over 360 lines.
    assert_eq!(SE0352N14TNGA0::WIDTH, 240);
    assert_eq!(SE0352N14TNGA0::HEIGHT, 360);
    assert_eq!(SE0352N14TNGA0::COLOR_MODE, ColorMode::TriColor);
}

/// Byte-for-byte parity with Waveshare's `EPD_3IN52B_Init()` and with the `ws_3in52b_init_code[]`
/// list in the Adafruit_EPD port. Also pins the RESOLUTION bytes derived from the controller's
/// configured dimensions to Waveshare's literal `[0xF0, 0x01, 0x68]`.
#[test]
fn test_uc8253_se0352n14_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
        .with_variant(Uc8253Variant::Se0352n14);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x04), // POWER_ON
            SpiRecord::Command(0x50), // CDI - 0x87, NOT the GDEY037T03 profile's 0x97
            SpiRecord::Data(vec![0x87]),
            SpiRecord::Command(0x00), // PANEL_SETTING
            SpiRecord::Data(vec![0x03, 0x0D]),
            SpiRecord::Command(0x61), // RESOLUTION - 240 x 0x0168 (360)
            SpiRecord::Data(vec![0xF0, 0x01, 0x68]),
            SpiRecord::Command(0x06), // BOOSTER_SOFT_START
            SpiRecord::Data(vec![0x2F, 0x2F, 0x2E]),
        ]
    );
}

/// The two variants put the Black/White plane on opposite RAM commands. Crossing them routes
/// black pixels into the red plane, so both directions are asserted together.
#[test]
fn test_uc8253_se0352n14_plane_routing() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
        .with_variant(Uc8253Variant::Se0352n14);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10), // WRITE_OLD_DATA carries Black/White here
            SpiRecord::Data(vec![0xAA, 0xBB]),
        ]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xCC])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x13), // WRITE_NEW_DATA carries Red here
            SpiRecord::Data(vec![0xCC]),
        ]
    );

    // write_frame_pattern must route identically to write_frame.
    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0x00, 2)
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x00, 0x00])]
    );

    // The default variant is untouched: Black/White stays on WRITE_NEW_DATA.
    bus_backend.records.borrow_mut().clear();
    let mut default_variant = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    assert_eq!(default_variant.variant(), Uc8253Variant::Gdey037t03);
    default_variant
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x13), SpiRecord::Data(vec![0xAA])]
    );
}

/// Refresh on this panel is a bare `DISPLAY_REFRESH`. Re-issuing `CDI` would move the DDX
/// polarity bits away from the `0x87` set at init and invert black and white, and the panel is
/// already powered from `init_sequence` — so neither may leak in, not even under a fast mode,
/// which this panel does not support at all.
#[test]
fn test_uc8253_se0352n14_trigger_refresh_ignores_refresh_mode() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    for mode in [
        Uc8253RefreshMode::Full,
        Uc8253RefreshMode::FastFull,
        Uc8253RefreshMode::Partial,
        Uc8253RefreshMode::FastPartial,
    ] {
        bus_backend.records.borrow_mut().clear();
        let mut controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
            .with_variant(Uc8253Variant::Se0352n14)
            .with_refresh_mode(mode);
        controller.trigger_refresh(&mut bus, &mut delay).unwrap();

        assert_eq!(
            bus_backend.records.borrow().clone(),
            vec![
                SpiRecord::Command(0x04), // POWER_ON — the charge pump drops after each update
                SpiRecord::Command(0x12), // DISPLAY_REFRESH
            ],
            "refresh mode {mode:?} leaked commands into the SE0352N14 refresh"
        );
    }
}

#[test]
fn test_uc8253_se0352n14_driver_smoke() {
    let spi = MockSpi;
    let dc = MockOutputPin;
    let rst = MockOutputPin;
    let busy = MockInputPin { is_high: true }; // Active-low BUSY: high means idle
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(spi, dc, rst, busy);
    let controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
        .with_variant(Uc8253Variant::Se0352n14);
    let mut driver = EpdBuilder::<_, SE0352N14TNGA0>::new(controller).build(bus);

    assert_eq!(driver.width(), 240);
    assert_eq!(driver.height(), 360);

    driver.init(&mut delay).expect("SE0352N14 init failed");
    // 0x00 is white in BOTH planes on this panel, unlike the monochrome GDEY037T03's 0xFF.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0x00)
        .expect("SE0352N14 black/white clear failed");
    driver
        .clear_frame(ColorChannel::RedYellow, 0x00)
        .expect("SE0352N14 red clear failed");
    driver
        .refresh(&mut delay)
        .expect("SE0352N14 refresh failed");
    driver.sleep(&mut delay).expect("SE0352N14 sleep failed");
}

/// A fixed settling delay after `DISPLAY_REFRESH` is not enough: BUSY assertion latency varies,
/// and a 10 ms guard was observed holding on some refreshes and missing on others on the same
/// XIAO ESP32-C3. Missing it leaves the panel a frame behind, because the caller writes into RAM
/// while the update is still running. `wait_busy_assert` waits for the edge instead of guessing.
#[test]
fn test_wait_busy_assert_waits_for_a_late_assertion() {
    /// BUSY that reads idle until `assert_after` polls have happened, then reads busy.
    ///
    /// Active-LOW, so "idle" is HIGH. Modelled on the real failure: the panel had not pulled BUSY
    /// down yet when the driver first looked.
    struct LateBusy {
        polls: RefCell<u32>,
        assert_after: u32,
    }

    impl DigitalErrorType for &LateBusy {
        type Error = core::convert::Infallible;
    }

    impl InputPin for &LateBusy {
        fn is_high(&mut self) -> Result<bool, Self::Error> {
            let mut polls = self.polls.borrow_mut();
            let now = *polls;
            *polls += 1;
            Ok(now < self.assert_after)
        }
        fn is_low(&mut self) -> Result<bool, Self::Error> {
            Ok(*self.polls.borrow() >= self.assert_after)
        }
    }

    struct CountingDelay(u32);
    impl DelayNs for CountingDelay {
        fn delay_ns(&mut self, _ns: u32) {}
        fn delay_ms(&mut self, ms: u32) {
            self.0 += ms;
        }
    }

    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);

    // Asserts only on the 26th poll — past any 10 ms fixed guard.
    let busy = LateBusy {
        polls: RefCell::new(0),
        assert_after: 25,
    };
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, &busy);
    let mut delay = CountingDelay(0);

    let asserted = bus.wait_busy_assert(&mut delay, false, 500).unwrap();
    assert!(asserted, "gave up before BUSY asserted");
    assert!(
        delay.0 >= 25,
        "returned after {} ms, before the panel asserted at 25 ms",
        delay.0
    );
}

/// A panel that never asserts must not hang the caller, so the wait is bounded and reports the
/// timeout rather than erroring — a missing panel still reads idle and falls straight through.
#[test]
fn test_wait_busy_assert_times_out_on_a_silent_panel() {
    struct CountingDelay(u32);
    impl DelayNs for CountingDelay {
        fn delay_ns(&mut self, _ns: u32) {}
        fn delay_ms(&mut self, ms: u32) {
            self.0 += ms;
        }
    }

    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    // MockInputPin reads HIGH: idle forever, for an active-LOW panel.
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, MockInputPin { is_high: true });
    let mut delay = CountingDelay(0);

    let asserted = bus.wait_busy_assert(&mut delay, false, 50).unwrap();
    assert!(!asserted, "claimed BUSY asserted on a silent panel");
    assert_eq!(delay.0, 50, "did not honour the timeout");
}

/// A fast update must restore the panel setting with a settling gap between the soft reset
/// (`0x1E`, RST_N low) and its release (`0x1F`). Issuing them back-to-back lets the reset's
/// power-on defaults win, which flips the scan direction and rotates every later frame.
#[test]
fn test_uc8253_fast_update_restores_panel_setting_with_settling_delay() {
    struct CountingDelay(u32);
    impl DelayNs for CountingDelay {
        fn delay_ns(&mut self, _ns: u32) {}
        fn delay_ms(&mut self, ms: u32) {
            self.0 += ms;
        }
    }

    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = CountingDelay(0);

    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::FastPartial);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();

    let records = bus_backend.records.borrow().clone();
    let tail = &records[records.len() - 4..];
    assert_eq!(
        tail,
        &[
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1E, 0x0D]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x1F, 0x0D]),
        ]
    );
    assert!(
        delay.0 >= 1,
        "no settling delay between the soft reset and its release"
    );
}

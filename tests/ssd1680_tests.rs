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

#[test]
fn test_ssd1680_gdem0213b74_panel_dimensions() {
    assert_eq!(GDEM0213B74::WIDTH, 122);
    assert_eq!(GDEM0213B74::HEIGHT, 250);
    assert_eq!(GxEPD2_213_B74::WIDTH, 122);
    assert_eq!(GxEPD2_213_B74::HEIGHT, 250);
}

#[test]
fn test_ssd1680_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1680Controller::new(GDEM0213B74::WIDTH, GDEM0213B74::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x12), // SW_RESET
            SpiRecord::Command(0x01), // DRIVER_CONTROL
            SpiRecord::Data(vec![0xF9, 0x00, 0x00]),
            SpiRecord::Command(0x3C), // BORDER_WAVEFORM_CONTROL
            SpiRecord::Data(vec![0x05]),
            SpiRecord::Command(0x21), // DISPLAY_UPDATE_CTRL1
            SpiRecord::Data(vec![0x00, 0x80]),
            SpiRecord::Command(0x18), // TEMP_CONTROL
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x11), // DATA_ENTRY_MODE
            SpiRecord::Data(vec![0x03]),
            SpiRecord::Command(0x44),                      // SET_RAMXPOS
            SpiRecord::Data(vec![0x00, 0x0F]),             // (122-1)/8 = 15 = 0x0F
            SpiRecord::Command(0x45),                      // SET_RAMYPOS
            SpiRecord::Data(vec![0x00, 0x00, 0xF9, 0x00]), // 249 = 0xF9
            SpiRecord::Command(0x4E),                      // SET_RAMXCNT
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT
            SpiRecord::Data(vec![0x00, 0x00]),
        ]
    );
}

#[test]
fn test_ssd1680_write_frame_channel_routing() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1680Controller::new(GDEM0213B74::WIDTH, GDEM0213B74::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x24), SpiRecord::Data(vec![0xAA, 0xBB]),]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xCC])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x26), SpiRecord::Data(vec![0xCC])]
    );
}

#[test]
fn test_ssd1680_trigger_refresh_full_and_partial() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // Full mode (default)
    let mut controller = Ssd1680Controller::new(GDEM0213B74::WIDTH, GDEM0213B74::HEIGHT);
    assert_eq!(controller.refresh_mode(), Ssd1680RefreshMode::Full);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xE0]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0x83]),
            SpiRecord::Command(0x20),
        ]
    );

    // Partial mode
    bus_backend.records.borrow_mut().clear();
    let mut controller = Ssd1680Controller::new(GDEM0213B74::WIDTH, GDEM0213B74::HEIGHT)
        .with_refresh_mode(Ssd1680RefreshMode::Partial);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xE0]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xFC]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0x83]),
            SpiRecord::Command(0x20),
        ]
    );
}

#[test]
fn test_ssd1680_gdey0266z90_panel_dimensions() {
    assert_eq!(GDEY0266Z90::WIDTH, 152);
    assert_eq!(GDEY0266Z90::HEIGHT, 296);
    assert_eq!(GxEPD2_266c::WIDTH, 152);
    assert_eq!(GxEPD2_266c::HEIGHT, 296);
}

#[test]
fn test_ssd1680_gdey0266z90_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1680Controller::new(GDEY0266Z90::WIDTH, GDEY0266Z90::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();

    // Same register set and byte values as GxEPD2_266c::_InitDisplay(); only the ordering of the
    // mutually independent 0x3C / 0x21 / 0x18 / 0x11 writes differs.
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x12),                // SW_RESET
            SpiRecord::Command(0x01),                // DRIVER_CONTROL
            SpiRecord::Data(vec![0x27, 0x01, 0x00]), // 296-1 = 295 = 0x0127
            SpiRecord::Command(0x3C),                // BORDER_WAVEFORM_CONTROL
            SpiRecord::Data(vec![0x05]),
            SpiRecord::Command(0x21), // DISPLAY_UPDATE_CTRL1
            SpiRecord::Data(vec![0x00, 0x80]),
            SpiRecord::Command(0x18), // TEMP_CONTROL
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x11), // DATA_ENTRY_MODE
            SpiRecord::Data(vec![0x03]),
            SpiRecord::Command(0x44),                      // SET_RAMXPOS
            SpiRecord::Data(vec![0x00, 0x12]),             // (152-1)/8 = 18 = 0x12
            SpiRecord::Command(0x45),                      // SET_RAMYPOS
            SpiRecord::Data(vec![0x00, 0x00, 0x27, 0x01]), // 295 = 0x0127
            SpiRecord::Command(0x4E),                      // SET_RAMXCNT
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT
            SpiRecord::Data(vec![0x00, 0x00]),
        ]
    );
}

#[test]
fn test_ssd1680_gdey0266z90_clear_frame_plane_polarity() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let controller = Ssd1680Controller::new(GDEY0266Z90::WIDTH, GDEY0266Z90::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEY0266Z90>::new(controller).build(bus);

    // 152 is byte-aligned: 19 bytes per row x 296 rows.
    let plane_bytes = 152 / 8 * 296;
    assert_eq!(plane_bytes, 5624);

    // The two planes disagree on ink polarity: 0xFF is white in the Black/White plane, but the
    // Red plane is inverted, so 0x00 is *no* red. GxEPD2 and both vendor drivers write ~color.
    for (channel, fill, expected_cmd) in [
        (ColorChannel::BlackWhite, 0xFFu8, 0x24u8),
        (ColorChannel::RedYellow, 0x00u8, 0x26u8),
    ] {
        bus_backend.records.borrow_mut().clear();
        driver.clear_frame(channel, fill).unwrap();
        let records = bus_backend.records.borrow().clone();

        assert_eq!(records[0], SpiRecord::Command(expected_cmd));

        // send_data_repeated chunks at 64 bytes, so assert the total rather than one record.
        let written: Vec<u8> = records[1..]
            .iter()
            .flat_map(|record| match record {
                SpiRecord::Data(bytes) => bytes.clone(),
                SpiRecord::Command(byte) => panic!("unexpected command 0x{byte:02X} in frame data"),
            })
            .collect();
        assert_eq!(written.len(), plane_bytes);
        assert!(written.iter().all(|&byte| byte == fill));
    }
}

#[test]
fn test_ssd1680_trigger_refresh_fast_full_and_base_map() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // FastFull: Good Display's temperature override (load sensor, force 90 C, reload the OTP LUT)
    // ahead of the 0xC7 display update, all inside the SSD1680 power envelope.
    let mut controller = Ssd1680Controller::new(GDEY0266Z90::WIDTH, GDEY0266Z90::HEIGHT)
        .with_refresh_mode(Ssd168xRefreshMode::FastFull);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xB1]), // load temperature from the internal sensor
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x1A),
            SpiRecord::Data(vec![0x5A, 0x00]), // override the register with 90 C
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0x91]), // reload the OTP LUT at that temperature
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xE0]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xC7]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0x83]),
            SpiRecord::Command(0x20),
        ]
    );

    // BaseMap: no preamble, just the 0xF4 base-map load.
    bus_backend.records.borrow_mut().clear();
    let mut controller = Ssd1680Controller::new(GDEY0266Z90::WIDTH, GDEY0266Z90::HEIGHT)
        .with_refresh_mode(Ssd168xRefreshMode::BaseMap);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xE0]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xF4]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0x83]),
            SpiRecord::Command(0x20),
        ]
    );
}

#[test]
fn test_ssd1680_sleep() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1680Controller::new(GDEM0213B74::WIDTH, GDEM0213B74::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x01])]
    );
}

// --- Characterisation of the init path (plan item 1b) ----------------------------------------
//
// `test_ssd1680_init_sequence` and `test_ssd1680_gdey0266z90_init_sequence` already pin the
// stream byte-for-byte. What was missing is the *absence* assertion: no panel-declared
// configuration reaches the wire today, because the `EpdPanel` hooks are `&self` methods on
// zero-sized types the controller never holds. The LUT upload turns that absence into a
// presence for panels that declare a LUT — and must leave it untouched for those that do not.

#[test]
fn test_ssd1680_init_writes_no_lut_or_vcom_today() {
    for (width, height) in [
        (GDEM0213B74::WIDTH, GDEM0213B74::HEIGHT),
        (GDEY0266Z90::WIDTH, GDEY0266Z90::HEIGHT),
    ] {
        let bus_backend = RecordingSpiBus::new();
        let dc = TestDc(&bus_backend);
        let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
        let mut controller = Ssd1680Controller::new(width, height);
        let mut delay = DummyDelay;

        controller.init_sequence(&mut bus, &mut delay).unwrap();

        let records = bus_backend.records.borrow().clone();
        for (command, name) in [
            (0x2Cu8, "VCOM"),
            (0x32, "LUT"),
            (0x03, "gate voltage"),
            (0x04, "source voltage"),
        ] {
            assert!(
                !records.contains(&SpiRecord::Command(command)),
                "{name} register ({command:#04X}) is never written today ({width}x{height})"
            );
        }
    }
}

// --- Panel config foundation (plan item 2d) --------------------------------------------------

fn record_ssd1680_init(controller: Ssd1680Controller) -> Vec<SpiRecord> {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = controller;
    let mut delay = DummyDelay;
    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();
    records
}

#[test]
fn test_ssd1680_for_panel_is_byte_identical_on_both_panels() {
    assert_eq!(
        record_ssd1680_init(Ssd1680Controller::for_panel::<GDEM0213B74>()),
        record_ssd1680_init(Ssd1680Controller::new(
            GDEM0213B74::WIDTH,
            GDEM0213B74::HEIGHT
        )),
    );
    assert_eq!(
        record_ssd1680_init(Ssd1680Controller::for_panel::<GDEY0266Z90>()),
        record_ssd1680_init(Ssd1680Controller::new(
            GDEY0266Z90::WIDTH,
            GDEY0266Z90::HEIGHT
        )),
    );
}

#[test]
fn test_ssd1680_for_panel_carries_the_refresh_mode_builder() {
    // `for_panel` must compose with the builders that already existed, not replace them.
    let controller = Ssd1680Controller::for_panel::<GDEY0266Z90>()
        .with_refresh_mode(Ssd1680RefreshMode::FastFull);
    assert_eq!(controller.refresh_mode(), Ssd1680RefreshMode::FastFull);
    assert_eq!(controller.vcom(), None);
}

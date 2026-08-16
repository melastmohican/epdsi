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
fn test_ssd1677_gdeq0426t82_panel_dimensions() {
    assert_eq!(GDEQ0426T82::WIDTH, 800);
    assert_eq!(GDEQ0426T82::HEIGHT, 480);
}

#[test]
fn test_ssd1677_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x12), // SW_RESET
            SpiRecord::Command(0x18), // TEMP_CONTROL
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x0C), // BOOSTER_SOFT_START
            SpiRecord::Data(vec![0xAE, 0xC7, 0xC3, 0xC0, 0x80]),
            SpiRecord::Command(0x01), // DRIVER_CONTROL
            SpiRecord::Data(vec![0xDF, 0x01, 0x02]),
            SpiRecord::Command(0x3C), // BORDER_WAVEFORM_CONTROL
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x11), // DATA_ENTRY_MODE (X+, Y-)
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x44), // SET_RAMXPOS
            SpiRecord::Data(vec![0x00, 0x63]), // 799/8 = 99 = 0x63
            SpiRecord::Command(0x45), // SET_RAMYPOS (end pair first, reversed)
            SpiRecord::Data(vec![0xDF, 0x01, 0x00, 0x00]),
            SpiRecord::Command(0x4E), // SET_RAMXCNT
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT
            SpiRecord::Data(vec![0xDF, 0x01]),
        ]
    );
}

#[test]
fn test_ssd1677_set_window_sub_rectangle_y_reversal() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    // x_start=100 (byte 12), x_end=199 (byte 24), y_start=50, y_end=99 (h=50).
    // yy = 480-50-50 = 380 = 0x017C, yy_end = 480-50-1 = 429 = 0x01AD.
    controller.set_window(&mut bus, 100, 50, 199, 99).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x44),
            SpiRecord::Data(vec![12, 24]),
            SpiRecord::Command(0x45),
            SpiRecord::Data(vec![0xAD, 0x01, 0x7C, 0x01]),
        ]
    );

    bus_backend.records.borrow_mut().clear();
    controller.set_cursor(&mut bus, 100, 50).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x4E),
            SpiRecord::Data(vec![12]),
            SpiRecord::Command(0x4F),
            SpiRecord::Data(vec![0xAD, 0x01]),
        ]
    );
}

#[test]
fn test_ssd1677_write_frame_channel_routing() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x24),
            SpiRecord::Data(vec![0xAA, 0xBB]),
        ]
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
fn test_ssd1677_trigger_refresh_all_modes() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // Full (default)
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    assert_eq!(controller.refresh_mode(), Ssd1677RefreshMode::Full);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x40, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x20),
        ]
    );

    // FastFull
    bus_backend.records.borrow_mut().clear();
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_refresh_mode(Ssd1677RefreshMode::FastFull);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x40, 0x00]),
            SpiRecord::Command(0x1A),
            SpiRecord::Data(vec![0x5A]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xD7]),
            SpiRecord::Command(0x20),
        ]
    );

    // Partial
    bus_backend.records.borrow_mut().clear();
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_refresh_mode(Ssd1677RefreshMode::Partial);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xFC]),
            SpiRecord::Command(0x20),
        ]
    );
}

#[test]
fn test_ssd1677_sleep() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x01])]
    );
}

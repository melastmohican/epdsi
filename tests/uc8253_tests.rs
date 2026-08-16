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

    // x_start=10 (rounds down to 8 via &0xFFF8), x_end=20 (rounds up to 23 via |0x0007)
    controller.set_window(&mut bus, 10, 5, 20, 15).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x91), // PARTIAL_IN
            SpiRecord::Command(0x90), // PARTIAL_WINDOW
            SpiRecord::Data(vec![8, 23, 0x00, 0x05, 0x00, 0x0F, 0x01]),
        ]
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
        vec![
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0xAA, 0xBB]),
        ]
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

    // Partial with a window set: PARTIAL_OUT first, CDI=0xD7.
    bus_backend.records.borrow_mut().clear();
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
        .with_refresh_mode(Uc8253RefreshMode::Partial);
    controller.set_window(&mut bus, 0, 0, 239, 415).unwrap();
    bus_backend.records.borrow_mut().clear();
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x92), // PARTIAL_OUT
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0xD7]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x02),
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

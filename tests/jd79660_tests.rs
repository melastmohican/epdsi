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
    dc_state: RefCell<bool>,
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
struct DummyRst;

impl DigitalErrorType for DummyRst {
    type Error = core::convert::Infallible;
}

impl OutputPin for DummyRst {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Active-low BUSY idle (HIGH) so `wait_busy(false)` returns immediately.
#[derive(Debug)]
struct IdleBusy;

impl DigitalErrorType for IdleBusy {
    type Error = core::convert::Infallible;
}

impl InputPin for IdleBusy {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(true)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

struct DummyDelay;

impl DelayNs for DummyDelay {
    fn delay_ns(&mut self, _ns: u32) {}
    fn delay_us(&mut self, _us: u32) {}
    fn delay_ms(&mut self, _ms: u32) {}
}

fn expected_init(fast: bool) -> Vec<SpiRecord> {
    let mut rec = vec![
        SpiRecord::Command(0x4D),
        SpiRecord::Data(vec![0x78]),
        SpiRecord::Command(0x00),
        SpiRecord::Data(vec![0x0F, 0x29]),
        SpiRecord::Command(0x06),
        SpiRecord::Data(vec![0x0D, 0x12, 0x30, 0x20, 0x19, 0x2A, 0x22]),
        SpiRecord::Command(0x50),
        SpiRecord::Data(vec![0x37]),
        SpiRecord::Command(0x61),
        SpiRecord::Data(vec![0x00, 0xC8, 0x00, 0xC8]),
        SpiRecord::Command(0xE9),
        SpiRecord::Data(vec![0x01]),
        SpiRecord::Command(0x30),
        SpiRecord::Data(vec![0x08]),
    ];
    if fast {
        rec.extend([
            SpiRecord::Command(0xE0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xE6),
            SpiRecord::Data(vec![0x5D]),
            SpiRecord::Command(0xA5),
            SpiRecord::Data(vec![0x00]),
        ]);
    }
    rec.extend([SpiRecord::Command(0x04)]);
    rec
}

fn expected_registers(fast: bool) -> Vec<SpiRecord> {
    let mut rec = expected_init(fast);
    rec.pop(); // drop PowerOn `0x04` — GxEPD2 skips it when `_power_is_on`
    rec
}

fn full_window_83() -> SpiRecord {
    SpiRecord::Data(vec![0x00, 0x00, 0x00, 199, 0x00, 0x00, 0x00, 199, 0x01])
}

#[test]
fn test_gdem0154f51h_panel_dimensions() {
    assert_eq!(GDEM0154F51H::WIDTH, 200);
    assert_eq!(GDEM0154F51H::HEIGHT, 200);
    assert_eq!(GxEPD2_154c_GDEM0154F51H::WIDTH, 200);
    assert_eq!(GDEM0154F51H::COLOR_MODE, ColorMode::QuadColor);
    assert_eq!(GDEM0154F51H::FRAME_BYTES, 10_000);
}

#[test]
fn test_jd79660_init_fast_full_update() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    assert_eq!(*bus_backend.records.borrow(), expected_init(true));
}

#[test]
fn test_jd79660_init_without_fast_full_update() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT)
        .with_fast_full_update(false);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    assert_eq!(*bus_backend.records.borrow(), expected_init(false));
}

#[test]
fn test_jd79660_write_frame() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .unwrap();
    assert_eq!(
        *bus_backend.records.borrow(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xAA, 0xBB]),]
    );
}

#[test]
fn test_jd79660_refresh() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        *bus_backend.records.borrow(),
        vec![
            SpiRecord::Command(0x83),
            full_window_83(),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x37]),
            SpiRecord::Command(0x12),
            SpiRecord::Data(vec![0x00]),
        ]
    );
}

#[test]
fn test_jd79660_sleep() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).unwrap();
    // No prior PowerOn — GxEPD2 `_PowerOff` is a no-op, then `hibernate` sends 0x07 0xA5.
    assert_eq!(
        *bus_backend.records.borrow(),
        vec![SpiRecord::Command(0x07), SpiRecord::Data(vec![0xA5])]
    );
}

#[test]
fn test_jd79660_sleep_after_init() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    bus_backend.records.borrow_mut().clear();
    controller.sleep(&mut bus, &mut delay).unwrap();
    assert_eq!(
        *bus_backend.records.borrow(),
        vec![
            SpiRecord::Command(0x02),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xA5]),
        ]
    );
}

#[test]
fn test_jd79660_partial_window() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);

    controller.set_window(&mut bus, 8, 16, 23, 31).unwrap();
    assert_eq!(
        *bus_backend.records.borrow(),
        vec![
            SpiRecord::Command(0x83),
            SpiRecord::Data(vec![0x00, 8, 0x00, 23, 0x00, 16, 0x00, 31, 0x01]),
        ]
    );
}

#[test]
fn test_jd79660_full_window_flag() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);

    controller.set_window(&mut bus, 0, 0, 199, 199).unwrap();
    assert_eq!(
        *bus_backend.records.borrow(),
        vec![
            SpiRecord::Command(0x83),
            SpiRecord::Data(vec![0x00, 0x00, 0x00, 199, 0x00, 0x00, 0x00, 199, 0x00]),
        ]
    );
}

#[test]
fn test_jd79660_refresh_partial_cdi() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.set_window(&mut bus, 8, 16, 23, 31).unwrap();
    bus_backend.records.borrow_mut().clear();
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        *bus_backend.records.borrow(),
        vec![
            SpiRecord::Command(0x83),
            SpiRecord::Data(vec![0x00, 8, 0x00, 23, 0x00, 16, 0x00, 31, 0x01]),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x97]),
            SpiRecord::Command(0x12),
            SpiRecord::Data(vec![0x00]),
        ]
    );
}

#[test]
fn test_jd79660_write_reinit_after_refresh() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA])
        .unwrap();

    let mut expected = expected_registers(true);
    expected.extend([SpiRecord::Command(0x10), SpiRecord::Data(vec![0xAA])]);
    assert_eq!(*bus_backend.records.borrow(), expected);
}

#[test]
fn test_jd79660_write_after_sleep_needs_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    controller.sleep(&mut bus, &mut delay).unwrap();
    bus_backend.records.borrow_mut().clear();
    // GxEPD2 would HW-reset here (`_hibernating`). `write_frame` has no DelayNs.
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA])
        .unwrap();
    assert_eq!(
        *bus_backend.records.borrow(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xAA])]
    );
}

#[test]
fn test_jd79660_init_after_sleep() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyRst, IdleBusy);
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    controller.sleep(&mut bus, &mut delay).unwrap();
    bus_backend.records.borrow_mut().clear();
    controller.init_sequence(&mut bus, &mut delay).unwrap();
    assert_eq!(*bus_backend.records.borrow(), expected_init(true));
}

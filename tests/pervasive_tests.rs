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
fn test_pervasive_controller_configuration() {
    let controller = PervasiveBwController::new(152, 296)
        .with_temperature(18)
        .with_refresh_mode(PervasiveRefreshMode::Fast)
        .with_psr([0xC0, 0x80])
        .with_auto_clear_secondary(false);

    assert_eq!(controller.temperature(), 18);
    assert_eq!(controller.refresh_mode(), PervasiveRefreshMode::Fast);

    let mut controller_mut = PervasiveBwController::new(152, 296);
    controller_mut.set_temperature(30);
    controller_mut.set_refresh_mode(PervasiveRefreshMode::Normal);
    assert_eq!(controller_mut.temperature(), 30);
    assert_eq!(controller_mut.refresh_mode(), PervasiveRefreshMode::Normal);
}

#[test]
fn test_pervasive_refresh_and_sleep_command_payload_parity() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwController::new(152, 296);
    let mut delay = DummyDelay;

    // Test trigger_refresh
    bus_backend.records.borrow_mut().clear();
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();

    let records = bus_backend.records.borrow().clone();
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x04), // POWER_ON
            SpiRecord::Command(0x12), // DISPLAY_REFRESH
        ]
    );

    // Test sleep
    bus_backend.records.borrow_mut().clear();
    controller.sleep(&mut bus, &mut delay).unwrap();

    let records = bus_backend.records.borrow().clone();
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x02), // POWER_OFF
        ]
    );
}

#[test]
fn test_pervasive_init_sequence_normal_vs_fast() {
    // Normal mode init sequence
    {
        let bus_backend = RecordingSpiBus::new();
        let dc = TestDc(&bus_backend);
        let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
        let mut controller = PervasiveBwController::new(152, 296).with_temperature(25);
        let mut delay = DummyDelay;

        controller.init_sequence(&mut bus, &mut delay).unwrap();
        let records = bus_backend.records.borrow().clone();

        assert_eq!(
            records,
            vec![
                SpiRecord::Command(0x00), // PSR (Soft Reset)
                SpiRecord::Data(vec![0x0E]),
                SpiRecord::Command(0xE5), // INPUT_TEMP
                SpiRecord::Data(vec![25]),
                SpiRecord::Command(0xE0), // ACTIVE_TEMP
                SpiRecord::Data(vec![0x02]),
                SpiRecord::Command(0x00), // PSR (Config)
                SpiRecord::Data(vec![0xCF, 0x8D]),
            ]
        );
    }

    // Fast mode init sequence
    {
        let bus_backend = RecordingSpiBus::new();
        let dc = TestDc(&bus_backend);
        let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
        let mut controller = PervasiveBwController::new(152, 296)
            .with_temperature(25)
            .with_refresh_mode(PervasiveRefreshMode::Fast);
        let mut delay = DummyDelay;

        controller.init_sequence(&mut bus, &mut delay).unwrap();
        let records = bus_backend.records.borrow().clone();

        assert_eq!(
            records,
            vec![
                SpiRecord::Command(0x00), // PSR (Soft Reset)
                SpiRecord::Data(vec![0x0E]),
                SpiRecord::Command(0xE5), // INPUT_TEMP (| 0x40 for fast)
                SpiRecord::Data(vec![25 | 0x40]),
                SpiRecord::Command(0xE0), // ACTIVE_TEMP
                SpiRecord::Data(vec![0x02]),
                SpiRecord::Command(0x00), // PSR (Config: 0xCF | 0x10, 0x8D | 0x02)
                SpiRecord::Data(vec![0xDF, 0x8F]),
                SpiRecord::Command(0x50), // VCOM_INTERVAL
                SpiRecord::Data(vec![0x07]),
            ]
        );
    }
}

#[test]
fn test_pervasive_write_frame_and_fast_frame() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwController::new(152, 296);

    let frame_data = [0xAA; 4];
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &frame_data)
        .unwrap();

    let records = bus_backend.records.borrow().clone();
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x10),                      // WRITE_BW_DATA
            SpiRecord::Data(vec![0x55, 0x55, 0x55, 0x55]), // !0xAA
            SpiRecord::Command(0x13),                      // WRITE_RED_DATA (Secondary clear)
            SpiRecord::Data(vec![0x00; 4]),
        ]
    );

    // Fast frame test
    bus_backend.records.borrow_mut().clear();
    let prev_frame = [0xFF; 4];
    let curr_frame = [0x00; 4];
    controller
        .write_fast_frame(&mut bus, &prev_frame, &curr_frame)
        .unwrap();

    let records = bus_backend.records.borrow().clone();
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x50), // VCOM_INTERVAL (Pre-write: 0x27)
            SpiRecord::Data(vec![0x27]),
            SpiRecord::Command(0x10), // WRITE_BW_DATA (Previous/Old)
            SpiRecord::Data(vec![0x00, 0x00, 0x00, 0x00]), // !0xFF
            SpiRecord::Command(0x13), // WRITE_RED_DATA (Current/New)
            SpiRecord::Data(vec![0xFF, 0xFF, 0xFF, 0xFF]), // !0x00
            SpiRecord::Command(0x50), // VCOM_INTERVAL (Post-write: 0x07)
            SpiRecord::Data(vec![0x07]),
        ]
    );
}

#[test]
fn test_pervasive_driver_f_e2290ks0f1_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwController::new(E2290KS0F1::WIDTH, E2290KS0F1::HEIGHT)
        .with_driver_variant(PervasiveDriverVariant::DriverF)
        .with_temperature(25);
    let mut delay = DummyDelay;

    assert_eq!(EPD_290_KS_0F::WIDTH, 168);
    assert_eq!(EPD_290_KS_0F::HEIGHT, 384);
    assert_eq!(EPD_266_KS_0C::WIDTH, 152);
    assert_eq!(EPD_266_KS_0C::HEIGHT, 296);

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x00), // PSR (Soft Reset)
            SpiRecord::Data(vec![0x0E]),
            SpiRecord::Command(0xE5), // INPUT_TEMP
            SpiRecord::Data(vec![25]),
            SpiRecord::Command(0xE0), // ACTIVE_TEMP
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0x4D), // Driver F register 0x4D
            SpiRecord::Data(vec![0x55]),
            SpiRecord::Command(0xE9), // Driver F register 0xE9
            SpiRecord::Data(vec![0x02]),
        ]
    );
}

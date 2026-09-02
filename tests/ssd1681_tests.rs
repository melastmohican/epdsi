#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only.

//! Unit and mock bus parity tests for SSD1681 E-Paper Display Controller.

use core::cell::RefCell;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
use embedded_hal::spi::{ErrorKind, ErrorType as SpiErrorType, Operation, SpiDevice};
use epdsi::controllers::ssd168x::{cmd, Ssd1681Controller, Ssd1681RefreshMode};
use epdsi::panels::GDEM0154Z90;
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
fn test_ssd1681_gdem0154z90_panel_dimensions() {
    assert_eq!(GDEM0154Z90::WIDTH, 200);
    assert_eq!(GDEM0154Z90::HEIGHT, 200);
}

#[test]
fn test_ssd1681_refresh_modes() {
    let mut controller = Ssd1681Controller::new(200, 200);
    assert_eq!(controller.refresh_mode(), Ssd1681RefreshMode::Full);

    controller.set_refresh_mode(Ssd1681RefreshMode::Partial);
    assert_eq!(controller.refresh_mode(), Ssd1681RefreshMode::Partial);

    let controller2 =
        Ssd1681Controller::new(200, 200).with_refresh_mode(Ssd1681RefreshMode::Partial);
    assert_eq!(controller2.refresh_mode(), Ssd1681RefreshMode::Partial);
}

#[test]
fn test_ssd1681_trigger_refresh_full_and_partial() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1681Controller::new(200, 200);
    let mut delay = DummyDelay;

    // Full refresh (0xF7)
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();
    assert!(records.contains(&SpiRecord::Command(cmd::UPDATE_DISPLAY_CTRL2)));
    assert!(records.contains(&SpiRecord::Data(vec![0xF7])));
    assert!(records.contains(&SpiRecord::Command(cmd::MASTER_ACTIVATE)));

    bus_backend.records.borrow_mut().clear();

    // Partial refresh (0xFC)
    controller.set_refresh_mode(Ssd1681RefreshMode::Partial);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();
    assert!(records.contains(&SpiRecord::Command(cmd::UPDATE_DISPLAY_CTRL2)));
    assert!(records.contains(&SpiRecord::Data(vec![0xFC])));
    assert!(records.contains(&SpiRecord::Command(cmd::MASTER_ACTIVATE)));
}

// --- Characterisation of the init path (plan item 1b) ----------------------------------------
//
// The SSD1681 init stream was previously unasserted. Pinning it byte-for-byte is what makes the
// coming LUT-upload change provable: a panel declaring no LUT must emit exactly this stream
// afterwards, with the `0x32` write appearing only for panels that declare one.

#[test]
fn test_ssd1681_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1681Controller::new(GDEM0154Z90::WIDTH, GDEM0154Z90::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x12), // SW_RESET
            SpiRecord::Command(0x01), // DRIVER_CONTROL: gate height 199 (0x00C7), scan 0x00
            SpiRecord::Data(vec![0xC7, 0x00, 0x00]),
            SpiRecord::Command(0x3C), // BORDER_WAVEFORM_CONTROL
            SpiRecord::Data(vec![0x05]),
            // No DISPLAY_UPDATE_CTRL1 (0x21) here — that write is SSD1680-only.
            SpiRecord::Command(0x18), // TEMP_CONTROL: internal sensor
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x11), // DATA_ENTRY_MODE: X increment, Y increment
            SpiRecord::Data(vec![0x03]),
            SpiRecord::Command(0x44), // SET_RAMXPOS: byte-valued, 0..=24 for 200 px
            SpiRecord::Data(vec![0x00, 0x18]),
            SpiRecord::Command(0x45), // SET_RAMYPOS: 16-bit start and end, 0..=199
            SpiRecord::Data(vec![0x00, 0x00, 0xC7, 0x00]),
            SpiRecord::Command(0x4E), // SET_RAMXCNT
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT
            SpiRecord::Data(vec![0x00, 0x00]),
        ]
    );
}

#[test]
fn test_ssd1681_init_writes_no_lut_or_vcom_today() {
    // GDEM0154Z90 declares `vcom() == Some(0x26)`, and the panel hooks are unreachable from the
    // controller, so nothing panel-specific reaches the wire. This asserts the *absence* that
    // Phase 2 turns into a presence — when `0x2C` starts appearing, this test is the one that
    // must be updated deliberately.
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1681Controller::new(GDEM0154Z90::WIDTH, GDEM0154Z90::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();

    let records = bus_backend.records.borrow().clone();
    assert!(
        !records.contains(&SpiRecord::Command(0x2C)),
        "VCOM register is never written today"
    );
    assert!(
        !records.contains(&SpiRecord::Command(0x32)),
        "LUT register is never written today"
    );
    assert!(
        !records.contains(&SpiRecord::Command(0x03)),
        "gate voltage register is never written today"
    );
}

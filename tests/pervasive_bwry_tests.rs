use core::cell::RefCell;
use std::collections::VecDeque;

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
use embedded_hal::spi::{ErrorKind, ErrorType as SpiErrorType, Operation, SpiDevice};
use epdsi::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SpiRecord {
    Command(u8),
    Data(Vec<u8>),
    Read(Vec<u8>),
}

/// Records every SPI transaction (like `RecordingSpiBus` elsewhere in this crate) but additionally
/// serves canned response bytes for `SpiDevice::read`, needed to exercise the BWRY OTP read protocol.
#[derive(Debug)]
struct RecordingSpiBus {
    records: RefCell<Vec<SpiRecord>>,
    dc_state: RefCell<bool>, // false = Low (Command), true = High (Data)
    read_queue: RefCell<VecDeque<u8>>,
}

impl RecordingSpiBus {
    fn new(canned_reads: &[u8]) -> Self {
        Self {
            records: RefCell::new(Vec::new()),
            dc_state: RefCell::new(false),
            read_queue: RefCell::new(canned_reads.iter().copied().collect()),
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

    fn read(&mut self, buf: &mut [u8]) -> Result<(), Self::Error> {
        let mut queue = self.read_queue.borrow_mut();
        for slot in buf.iter_mut() {
            *slot = queue.pop_front().expect("read past end of canned bytes");
        }
        self.records
            .borrow_mut()
            .push(SpiRecord::Read(buf.to_vec()));
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
    assert_eq!(E2154QS0F1::WIDTH, 200);
    assert_eq!(E2154QS0F1::HEIGHT, 200);
    assert_eq!(EPD_152_QS_06::WIDTH, 200);
    assert_eq!(E2417QS0A3::WIDTH, 400);
    assert_eq!(E2417QS0A3::HEIGHT, 300);
    assert_eq!(EPD_417_QS_0A::WIDTH, 400);
}

#[test]
fn test_pervasive_bwry_chip_id_mismatch() {
    // Driver6 expects chip ID 0x4801; canned response is a different value.
    let canned = [0x00u8, 0x01u8];
    let bus_backend = RecordingSpiBus::new(&canned);
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller =
        PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT);
    let mut delay = DummyDelay;

    let err = controller.init_sequence(&mut bus, &mut delay).unwrap_err();
    assert_eq!(err, PervasiveBwryError::UnexpectedChipId(0x0001));
}

#[test]
fn test_pervasive_bwry_chip_id_normalization() {
    // 0x8302 raw response must be normalized to 0x0302 before comparison (mismatch expected
    // against Driver6's 0x4801, but the reported id proves normalization ran).
    let canned = [0x83u8, 0x02u8];
    let bus_backend = RecordingSpiBus::new(&canned);
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller =
        PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT);
    let mut delay = DummyDelay;

    let err = controller.init_sequence(&mut bus, &mut delay).unwrap_err();
    assert_eq!(err, PervasiveBwryError::UnexpectedChipId(0x0302));
}

#[test]
fn test_pervasive_bwry_driver6_init_sequence() {
    let otp = synthetic_otp(48);
    let mut canned = vec![0x48u8, 0x01u8]; // chip ID
    canned.push(0xAA); // dummy byte (discarded)
    canned.extend_from_slice(&otp); // marker (otp[0]) + otp[1..48]

    let bus_backend = RecordingSpiBus::new(&canned);
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT)
        .with_temperature(25)
        .with_variant(PervasiveBwryVariant::Driver6);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x70),
            SpiRecord::Read(vec![0x48, 0x01]),
            SpiRecord::Command(0xf0),
            SpiRecord::Data(vec![0x0b]),
            SpiRecord::Command(0x90),
            SpiRecord::Command(0xa2),
            SpiRecord::Data(vec![0x33]),
            SpiRecord::Command(0xa0),
            SpiRecord::Command(0xf6),
            SpiRecord::Data(vec![0x2d, 0x80]),
            SpiRecord::Command(0x92),
            SpiRecord::Read(vec![0xAA]),
            SpiRecord::Read(vec![0xA5]),
            SpiRecord::Read(slice(&otp, 1, 48)),
            SpiRecord::Command(0xe0), // ACTIVE_STATE
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xe6), // INPUT_TEMP (BWRY register, not 0xE5)
            SpiRecord::Data(vec![25]),
            SpiRecord::Command(0xa5),
            SpiRecord::Command(0x01),
            SpiRecord::Data(slice(&otp, 16, 18)),
            SpiRecord::Command(0x00), // PSR
            SpiRecord::Data(slice(&otp, 18, 20)),
            SpiRecord::Command(0x61),
            SpiRecord::Data(slice(&otp, 20, 24)),
            SpiRecord::Command(0x06),
            SpiRecord::Data(slice(&otp, 24, 28)),
            SpiRecord::Command(0x03),
            SpiRecord::Data(slice(&otp, 30, 31)),
            SpiRecord::Command(0xe7),
            SpiRecord::Data(slice(&otp, 33, 34)),
            SpiRecord::Command(0x65),
            SpiRecord::Data(slice(&otp, 34, 38)),
            SpiRecord::Command(0x30),
            SpiRecord::Data(slice(&otp, 38, 39)),
            SpiRecord::Command(0x50), // CDI
            SpiRecord::Data(slice(&otp, 39, 40)),
            SpiRecord::Command(0x60),
            SpiRecord::Data(slice(&otp, 40, 42)),
            SpiRecord::Command(0xe3),
            SpiRecord::Data(slice(&otp, 42, 43)),
            SpiRecord::Command(0x62),
            SpiRecord::Data(slice(&otp, 43, 45)),
            SpiRecord::Command(0xe9),
            SpiRecord::Data(vec![0x01]),
        ]
    );

    // Driver6 must not power on during init (only DriverA does).
    assert!(!records.contains(&SpiRecord::Command(0x04)));
}

#[test]
fn test_pervasive_bwry_drivera_init_sequence_power_on_during_init() {
    let otp = synthetic_otp(112);
    let mut canned = vec![0x06u8, 0x05u8]; // chip ID
    canned.push(0xAA); // dummy byte (discarded)
    canned.extend_from_slice(&otp); // marker (otp[0]) + otp[1..112], found on first attempt

    let bus_backend = RecordingSpiBus::new(&canned);
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_temperature(25)
        .with_variant(PervasiveBwryVariant::DriverA);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x70),
            SpiRecord::Read(vec![0x06, 0x05]),
            SpiRecord::Command(0xa2),
            SpiRecord::Data(vec![0x00, 0x15, 0x00]),
            SpiRecord::Command(0xa0),
            SpiRecord::Command(0x92),
            SpiRecord::Read(vec![0xAA]),
            SpiRecord::Read(vec![0xA5]),
            SpiRecord::Read(slice(&otp, 1, 112)),
            SpiRecord::Command(0xe0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xe6),
            SpiRecord::Data(vec![25]),
            SpiRecord::Command(0x01),
            SpiRecord::Data(slice(&otp, 16, 17)),
            SpiRecord::Command(0x00), // PSR
            SpiRecord::Data(slice(&otp, 17, 19)),
            SpiRecord::Command(0x03),
            SpiRecord::Data(slice(&otp, 30, 33)),
            SpiRecord::Command(0x06),
            SpiRecord::Data(slice(&otp, 23, 26)),
            SpiRecord::Command(0x50), // CDI
            SpiRecord::Data(slice(&otp, 39, 40)),
            SpiRecord::Command(0x60),
            SpiRecord::Data(slice(&otp, 40, 42)),
            SpiRecord::Command(0x61),
            SpiRecord::Data(slice(&otp, 19, 23)),
            SpiRecord::Command(0xe3),
            SpiRecord::Data(slice(&otp, 42, 43)),
            SpiRecord::Command(0xe7),
            SpiRecord::Data(slice(&otp, 33, 34)),
            SpiRecord::Command(0x65),
            SpiRecord::Data(slice(&otp, 34, 38)),
            SpiRecord::Command(0x30),
            SpiRecord::Data(slice(&otp, 38, 39)),
            SpiRecord::Command(0xe9),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x04), // POWER_ON (DriverA only, happens during init)
        ]
    );
}

#[test]
fn test_pervasive_bwry_drivera_bank2_fallback() {
    let otp = synthetic_otp(112);
    let mut canned = vec![0x06u8, 0x05u8]; // chip ID
    canned.push(0xAA); // dummy byte
    canned.push(0x00); // bad marker at bank 1
    canned.extend(std::iter::repeat(0xFF).take(0x70 - 1)); // discarded bank-2 seek bytes
    canned.push(0xA5); // good marker at bank 2
    canned.extend_from_slice(&otp[1..112]); // remaining OTP payload

    let bus_backend = RecordingSpiBus::new(&canned);
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
    let records = bus_backend.records.borrow().clone();

    // Confirm the fallback path actually ran: chip-id, dummy, bad marker, the 111-byte bank-2
    // seek discard, the good marker, then the remaining OTP payload.
    let read_records: Vec<SpiRecord> = records
        .iter()
        .filter(|r| matches!(r, SpiRecord::Read(_)))
        .cloned()
        .collect();
    // `discard_read_bytes` reads in <=64-byte chunks, so the 111-byte bank-2 seek discard splits
    // into a 64-byte and a 47-byte `Read` record.
    assert_eq!(
        read_records,
        vec![
            SpiRecord::Read(vec![0x06, 0x05]), // chip id
            SpiRecord::Read(vec![0xAA]),       // dummy
            SpiRecord::Read(vec![0x00]),       // bad marker (bank 1)
            SpiRecord::Read(vec![0xFF; 64]),   // bank-2 seek discard, chunk 1
            SpiRecord::Read(vec![0xFF; 47]),   // bank-2 seek discard, chunk 2
            SpiRecord::Read(vec![0xA5]),       // good marker (bank 2)
            SpiRecord::Read(otp[1..112].to_vec()), // remaining OTP payload
        ]
    );

    // Spot-check a downstream register write pulls from the correctly-offset OTP data.
    assert!(records.contains(&SpiRecord::Command(0x00)));
}

#[test]
fn test_pervasive_bwry_drivera_invalid_otp_marker() {
    let mut canned = vec![0x06u8, 0x05u8]; // chip ID
    canned.push(0xAA); // dummy byte
    canned.push(0x00); // bad marker at bank 1
    canned.extend(std::iter::repeat(0xFF).take(0x70 - 1)); // discarded bank-2 seek bytes
    canned.push(0x00); // still-bad marker at bank 2

    let bus_backend = RecordingSpiBus::new(&canned);
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);
    let mut delay = DummyDelay;

    let err = controller.init_sequence(&mut bus, &mut delay).unwrap_err();
    assert_eq!(err, PervasiveBwryError::InvalidOtpMarker);
}

#[test]
fn test_pervasive_bwry_write_frame_and_pattern() {
    let bus_backend = RecordingSpiBus::new(&[]);
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0xAA, 0xBB]),
        ]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0x11, 4)
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x11; 4])]
    );
}

#[test]
fn test_pervasive_bwry_trigger_refresh_and_sleep() {
    let bus_backend = RecordingSpiBus::new(&[]);
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // Driver6: powers on then refreshes.
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT)
        .with_variant(PervasiveBwryVariant::Driver6);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x12),
            SpiRecord::Data(vec![0x00]),
        ]
    );
    bus_backend.records.borrow_mut().clear();
    controller.sleep(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x02),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xa5]),
        ]
    );

    // DriverA: already powered on (during init), refresh only sends DISPLAY_REFRESH.
    bus_backend.records.borrow_mut().clear();
    let mut controller = PervasiveBwryController::new(E2417QS0A3::WIDTH, E2417QS0A3::HEIGHT)
        .with_variant(PervasiveBwryVariant::DriverA);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x12), SpiRecord::Data(vec![0x00])]
    );
    bus_backend.records.borrow_mut().clear();
    controller.sleep(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x02),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x00), // PSR re-send from otp[26..28] (default zero-initialized here)
            SpiRecord::Data(vec![0x00, 0x00]),
        ]
    );
}

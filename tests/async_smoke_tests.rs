#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
#![cfg(not(feature = "blocking"))]
//! Async-mode coverage: one smoke test per controller file, run with
//! `cargo test --no-default-features --features graphics`. Not a duplicate of the full blocking
//! suite (see the plan's decision #6) — just enough to prove the `maybe_async_cfg` split didn't
//! diverge from the pinned blocking byte streams in `tests/{ssd1680,ssd1677,uc8253,pervasive}_tests.rs`.

use core::cell::RefCell;
use embedded_hal::digital::{ErrorType as DigitalErrorType, OutputPin};
use embedded_hal::spi::{ErrorKind, ErrorType as SpiErrorType};
use embedded_hal_async::digital::Wait;
use embedded_hal_async::spi::{Operation, SpiDevice};
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
    async fn transaction(
        &mut self,
        _operations: &mut [Operation<'_, u8>],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        let is_data = *self.dc_state.borrow();
        if is_data {
            self.records.borrow_mut().push(SpiRecord::Data(buf.to_vec()));
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
impl Wait for DummyPin {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
// `SpiBusWrapper`'s always-present accessor block (`busy_is_high`) still requires `InputPin`
// alongside `Wait` in async mode — see its module doc.
impl embedded_hal::digital::InputPin for DummyPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

struct DummyDelay;
impl embedded_hal_async::delay::DelayNs for DummyDelay {
    async fn delay_ns(&mut self, _ns: u32) {}
    async fn delay_us(&mut self, _us: u32) {}
    async fn delay_ms(&mut self, _ms: u32) {}
}

#[test]
fn test_ssd1680_init_sequence_matches_blocking() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1680Controller::new(GDEM0213B74::WIDTH, GDEM0213B74::HEIGHT);
    let mut delay = DummyDelay;

    pollster::block_on(controller.init_sequence(&mut bus, &mut delay)).unwrap();
    let records = bus_backend.records.borrow().clone();

    // Pinned in tests/ssd1680_tests.rs::test_ssd1680_init_sequence (blocking mode).
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x01),
            SpiRecord::Data(vec![0xF9, 0x00, 0x00]),
            SpiRecord::Command(0x3C),
            SpiRecord::Data(vec![0x05]),
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x00, 0x80]),
            SpiRecord::Command(0x18),
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x11),
            SpiRecord::Data(vec![0x03]),
            SpiRecord::Command(0x44),
            SpiRecord::Data(vec![0x00, 0x0F]),
            SpiRecord::Command(0x45),
            SpiRecord::Data(vec![0x00, 0x00, 0xF9, 0x00]),
            SpiRecord::Command(0x4E),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x4F),
            SpiRecord::Data(vec![0x00, 0x00]),
        ]
    );
}

#[test]
fn test_ssd1677_init_sequence_matches_blocking() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut delay = DummyDelay;

    pollster::block_on(controller.init_sequence(&mut bus, &mut delay)).unwrap();
    let records = bus_backend.records.borrow().clone();

    // Pinned in tests/ssd1677_tests.rs::test_ssd1677_init_sequence (blocking mode).
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x12),
            SpiRecord::Command(0x18),
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x0C),
            SpiRecord::Data(vec![0xAE, 0xC7, 0xC3, 0xC0, 0x80]),
            SpiRecord::Command(0x01),
            SpiRecord::Data(vec![0xDF, 0x01, 0x02]),
            SpiRecord::Command(0x3C),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x11),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x44),
            SpiRecord::Data(vec![0x00, 0x00, 0x1F, 0x03]),
            SpiRecord::Command(0x45),
            SpiRecord::Data(vec![0xDF, 0x01, 0x00, 0x00]),
            SpiRecord::Command(0x4E),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x4F),
            SpiRecord::Data(vec![0xDF, 0x01]),
        ]
    );
}

#[test]
fn test_uc8253_init_sequence_matches_blocking() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT);
    let mut delay = DummyDelay;

    pollster::block_on(controller.init_sequence(&mut bus, &mut delay)).unwrap();
    let records = bus_backend.records.borrow().clone();

    // Pinned in tests/uc8253_tests.rs::test_uc8253_init_sequence (blocking mode).
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
fn test_pervasive_bw_init_sequence_matches_blocking() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwController::new(152, 296).with_temperature(25);
    let mut delay = DummyDelay;

    pollster::block_on(controller.init_sequence(&mut bus, &mut delay)).unwrap();
    let records = bus_backend.records.borrow().clone();

    // Pinned in tests/pervasive_tests.rs::test_pervasive_init_sequence_normal_vs_fast
    // (blocking mode, Normal-mode branch).
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x0E]),
            SpiRecord::Command(0xE5),
            SpiRecord::Data(vec![25]),
            SpiRecord::Command(0xE0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0xCF, 0x8D]),
        ]
    );
}

#[test]
fn test_pervasive_bwry_init_sequence_matches_blocking() {
    // No `read_otp` call — `otp_data` defaults to all-zero, so every OTP-derived `Data` payload
    // below is zero-filled; the command bytes and slice lengths are what this test pins.
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT)
        .with_temperature(25)
        .with_variant(PervasiveBwryVariant::DriverF);
    let mut delay = DummyDelay;

    pollster::block_on(controller.init_sequence(&mut bus, &mut delay)).unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0xe0),
            SpiRecord::Data(vec![0x02]),
            SpiRecord::Command(0xe6),
            SpiRecord::Data(vec![25]),
            SpiRecord::Command(0xa5),
            SpiRecord::Command(0x01),
            SpiRecord::Data(vec![0x00; 1]),
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x00; 2]),
            SpiRecord::Command(0x03),
            SpiRecord::Data(vec![0x00; 3]),
            SpiRecord::Command(0x06),
            SpiRecord::Data(vec![0x00; 7]),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x00; 1]),
            SpiRecord::Command(0x60),
            SpiRecord::Data(vec![0x00; 2]),
            SpiRecord::Command(0x61),
            SpiRecord::Data(vec![0x00; 4]),
            SpiRecord::Command(0xe7),
            SpiRecord::Data(vec![0x00; 1]),
            SpiRecord::Command(0xe3),
            SpiRecord::Data(vec![0x00; 1]),
            SpiRecord::Command(0x4d),
            SpiRecord::Data(vec![0x00; 1]),
            SpiRecord::Command(0xb4),
            SpiRecord::Data(vec![0x00; 1]),
            SpiRecord::Command(0xb5),
            SpiRecord::Data(vec![0x00; 1]),
            SpiRecord::Command(0xe9),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x30),
            SpiRecord::Data(vec![0x08]),
        ]
    );
}

#[test]
fn test_jd79661_full_driver_flow_does_not_error() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let controller = Jd79661Controller::new(
        ZJY122250_0213AJH_E5::WIDTH,
        ZJY122250_0213AJH_E5::HEIGHT,
    );
    let mut driver = EpdBuilder::<_, ZJY122250_0213AJH_E5>::new(controller).build(bus);
    let mut delay = DummyDelay;

    pollster::block_on(async {
        driver.init(&mut delay).await.unwrap();
        driver
            .clear_frame(ColorChannel::BlackWhite, 0x00)
            .await
            .unwrap();
        driver.refresh(&mut delay).await.unwrap();
        driver.sleep(&mut delay).await.unwrap();
    });
}

#[test]
fn test_ed2208_full_driver_flow_does_not_error() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEP073E01>::new(controller).build(bus);
    let mut delay = DummyDelay;

    pollster::block_on(async {
        driver.init(&mut delay).await.unwrap();
        let white_packed = SevenColor::pack(SevenColor::White, SevenColor::White);
        driver
            .clear_frame(ColorChannel::Color7(0), white_packed)
            .await
            .unwrap();
        driver.refresh(&mut delay).await.unwrap();
        driver.sleep(&mut delay).await.unwrap();
    });
}

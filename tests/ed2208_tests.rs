#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
use embedded_hal::spi::{ErrorKind, ErrorType as SpiErrorType, Operation, SpiDevice};
use epdsi::prelude::*;

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

#[derive(Debug)]
struct MockDelay;

impl DelayNs for MockDelay {
    fn delay_ns(&mut self, _ns: u32) {}
    fn delay_us(&mut self, _us: u32) {}
    fn delay_ms(&mut self, _ms: u32) {}
}

#[test]
fn test_ed2208_gdep073e01_instantiation_and_rendering() {
    let spi = MockSpi;
    let dc = MockOutputPin;
    let rst = MockOutputPin;
    let busy = MockInputPin { is_high: true }; // Active-low BUSY: high means idle
    let mut delay = MockDelay;

    let bus = SpiBusWrapper::new(spi, dc, rst, busy);
    let controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEP073E01>::new(controller).build(bus);

    assert_eq!(driver.width(), 800);
    assert_eq!(driver.height(), 480);

    // Test initialization
    driver.init(&mut delay).expect("ED2208 init failed");

    // Test clearing frame to white (0x11 = white packed)
    let white_packed = SevenColor::pack(SevenColor::White, SevenColor::White);
    assert_eq!(white_packed, 0x11);
    driver
        .clear_frame(ColorChannel::Color7(0), white_packed)
        .expect("ED2208 clear frame failed");

    // Test refresh and sleep
    driver.refresh(&mut delay).expect("ED2208 refresh failed");
    driver.sleep(&mut delay).expect("ED2208 sleep failed");
}

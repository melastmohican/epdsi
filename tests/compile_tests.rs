use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
use embedded_hal::spi::{ErrorKind, Operation, SpiDevice, ErrorType as SpiErrorType};
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
struct MockInputPin;

impl DigitalErrorType for MockInputPin {
    type Error = core::convert::Infallible;
}

impl InputPin for MockInputPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }

    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(true)
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
fn test_ssd1681_epd_driver_instantiation_and_paged_rendering() {
    let spi = MockSpi;
    let dc = MockOutputPin;
    let rst = MockOutputPin;
    let busy = MockInputPin;
    let mut delay = MockDelay;

    let bus = SpiBusWrapper::new(spi, dc, rst, busy);
    let controller = Ssd1681Controller::new(GDEM0154Z90::WIDTH, GDEM0154Z90::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEM0154Z90>::new(controller).build(bus);

    assert_eq!(driver.width(), 200);
    assert_eq!(driver.height(), 200);

    // Test initialization
    driver.init(&mut delay).expect("Initialization failed");

    // Test clear frame
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .expect("Clear frame failed");

    // Test paged rendering
    let mut page_buffer = [0u8; (200 * 20) / 8];
    render_paged(
        &mut driver,
        &mut delay,
        ColorChannel::BlackWhite,
        &mut page_buffer,
        20,
        0xFF,
        |page_buf| {
            page_buf.set_pixel(10, page_buf.y_offset() + 5, true);
        },
    )
    .expect("Paged rendering failed");
}

#[test]
fn test_jd79661_epd_driver_instantiation_and_paged_rendering() {
    let spi = MockSpi;
    let dc = MockOutputPin;
    let rst = MockOutputPin;
    let busy = MockInputPin;
    let mut delay = MockDelay;

    let bus = SpiBusWrapper::new(spi, dc, rst, busy);
    let controller = Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);
    let mut driver = EpdBuilder::<_, ZJY122250_0213AJH_E5>::new(controller).build(bus);

    assert_eq!(driver.width(), 250);
    assert_eq!(driver.height(), 122);

    // Test initialization
    driver.init(&mut delay).expect("Initialization failed");

    // Test clear frame
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .expect("Clear frame failed");

    // Test paged rendering
    let mut page_buffer = [0u8; (250 * 20) / 8];
    render_paged(
        &mut driver,
        &mut delay,
        ColorChannel::BlackWhite,
        &mut page_buffer,
        20,
        0xFF,
        |page_buf| {
            page_buf.set_pixel(10, page_buf.y_offset() + 5, true);
        },
    )
    .expect("Paged rendering failed");
}

//! JD79661 E-Paper Display Controller implementation.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// JD79661 Command Definitions
pub mod cmd {
    /// Panel Setting command
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power Setting / Software Reset command
    pub const POWER_SETTING: u8 = 0x01;
    /// Power OFF command
    pub const POWER_OFF: u8 = 0x02;
    /// Power Offset command
    pub const POWER_OFFSET: u8 = 0x03;
    /// Power ON command
    pub const POWER_ON: u8 = 0x04;
    /// Booster Soft Start command
    pub const BOOSTER_SOFT_START: u8 = 0x06;
    /// Deep Sleep command
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Data Start Transmission / Write RAM
    pub const DATA_START_TRANSMISSION: u8 = 0x10;
    /// Display Refresh command
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// VCOM / Power Control
    pub const VCOM_CONTROL: u8 = 0x30;
    /// Vendor Magic Key
    pub const MAGIC_KEY: u8 = 0x4D;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// TCON Setting
    pub const TCON: u8 = 0x60;
    /// Resolution Setting
    pub const RESOLUTION: u8 = 0x61;
}

/// JD79661 Controller IC driver implementation.
#[derive(Debug, Clone, Copy)]
pub struct Jd79661Controller {
    width: u32,
    height: u32,
}

impl Jd79661Controller {
    /// Creates a new JD79661 controller configured for target display dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Jd79661Controller
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    type Error = EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;

    fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // Hardware reset sequence
        bus.hard_reset(delay, 10)?;

        // Software reset / init wait (JD79661 busy is active-low: busy when LOW)
        bus.wait_busy(false)?;
        bus.send_command_with_data(cmd::POWER_SETTING, &[])?;
        bus.wait_busy(false)?;

        // Vendor unlock key
        bus.send_command_with_data(cmd::MAGIC_KEY, &[0x78])?;

        // Panel Setting (128x250)
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x8F, 0x29])?;

        // Power setting
        bus.send_command_with_data(cmd::POWER_SETTING, &[0x07, 0x00])?;

        // Power offset
        bus.send_command_with_data(cmd::POWER_OFFSET, &[0x10, 0x54, 0x44])?;

        // Booster Soft Start
        bus.send_command_with_data(
            cmd::BOOSTER_SOFT_START,
            &[0x05, 0x00, 0x3F, 0x0A, 0x25, 0x12, 0x1A],
        )?;

        // CDI (VCOM and Data Interval Setting)
        bus.send_command_with_data(cmd::CDI, &[0x37])?;

        // TCON
        bus.send_command_with_data(cmd::TCON, &[0x02, 0x02, 0x02])?;

        // Resolution setting (128 x 250)
        let w_high = ((self.width >> 8) & 0xFF) as u8;
        let w_low = (self.width & 0xFF) as u8;
        let h_high = ((self.height >> 8) & 0xFF) as u8;
        let h_low = (self.height & 0xFF) as u8;
        bus.send_command_with_data(cmd::RESOLUTION, &[w_high, w_low, h_high, h_low])?;

        // Additional vendor config registers
        bus.send_command_with_data(0xE7, &[0x1C])?;
        bus.send_command_with_data(0xE3, &[0x22])?;
        bus.send_command_with_data(0xB4, &[0xD0])?;
        bus.send_command_with_data(0xB5, &[0x03])?;
        bus.send_command_with_data(0xE9, &[0x01])?;
        bus.send_command_with_data(cmd::VCOM_CONTROL, &[0x08])?;

        // Power ON and wait until ready
        bus.send_command(cmd::POWER_ON)?;
        bus.wait_busy(false)?;

        Ok(())
    }

    fn set_window(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x_start: u32,
        _y_start: u32,
        _x_end: u32,
        _y_end: u32,
    ) -> Result<(), Self::Error> {
        // JD79661 uses full frame buffer streaming by default
        Ok(())
    }

    fn set_cursor(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x: u32,
        _y: u32,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::DATA_START_TRANSMISSION, data)
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::DATA_START_TRANSMISSION)?;
        bus.send_data_repeated(byte, count)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::DISPLAY_REFRESH)?;
        bus.wait_busy(false)
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::POWER_OFF)?;
        bus.wait_busy(false)?;
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5])
    }
}

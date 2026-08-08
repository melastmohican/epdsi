//! Pervasive Displays E-Paper Display Controller implementation.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// Pervasive Displays Command Register Definitions
pub mod cmd {
    /// Panel Setting Register (PSR)
    pub const PSR: u8 = 0x00;
    /// Power Off command
    pub const POWER_OFF: u8 = 0x02;
    /// Power On command
    pub const POWER_ON: u8 = 0x04;
    /// Write Black/White RAM data (BufferBlack / DTM1)
    pub const WRITE_BW_DATA: u8 = 0x10;
    /// Display Refresh command (DRF)
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// Write Red/Yellow RAM data (BufferRed / DTM2)
    pub const WRITE_RED_DATA: u8 = 0x13;
    /// Active Temperature sensor selection
    pub const ACTIVE_TEMP: u8 = 0xE0;
    /// Input Temperature value selection
    pub const INPUT_TEMP: u8 = 0xE5;
}

// Register configuration constants
const REG_DATA_SOFT_RESET: &[u8] = &[0x0E];
const REG_DATA_INPUT_TEMP: &[u8] = &[0x19];
const REG_DATA_ACTIVE_TEMP: &[u8] = &[0x02];
const REG_DATA_PSR_CONFIG: &[u8] = &[0xCF, 0x8D];

/// Pervasive Displays COG Controller IC driver implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PervasiveDisplaysController {
    width: u32,
    height: u32,
}

impl PervasiveDisplaysController {
    /// Creates a new Pervasive Displays controller instance with target resolution.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for PervasiveDisplaysController
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

        // Pervasive Displays busy pin is active-low (busy when LOW)
        bus.wait_busy(false)?;

        // Soft reset command
        bus.send_command_with_data(cmd::PSR, REG_DATA_SOFT_RESET)?;
        bus.wait_busy(false)?;

        // Temperature calibration
        bus.send_command_with_data(cmd::INPUT_TEMP, REG_DATA_INPUT_TEMP)?;
        bus.send_command_with_data(cmd::ACTIVE_TEMP, REG_DATA_ACTIVE_TEMP)?;

        // Panel setting configuration
        bus.send_command_with_data(cmd::PSR, REG_DATA_PSR_CONFIG)?;

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
        // Full frame streaming used by Pervasive displays
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
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        match channel {
            ColorChannel::BlackWhite => {
                bus.send_command(cmd::WRITE_BW_DATA)?;
                let mut buf = [0u8; 64];
                for chunk in data.chunks(64) {
                    for (i, &b) in chunk.iter().enumerate() {
                        buf[i] = !b;
                    }
                    bus.send_data(&buf[..chunk.len()])?;
                }
                Ok(())
            }
            ColorChannel::RedYellow | ColorChannel::Red | ColorChannel::Yellow | ColorChannel::Color7(_) => {
                bus.send_command_with_data(cmd::WRITE_RED_DATA, data)
            }
        }
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        let (cmd, byte) = match channel {
            ColorChannel::BlackWhite => (cmd::WRITE_BW_DATA, !byte),
            ColorChannel::RedYellow | ColorChannel::Red | ColorChannel::Yellow | ColorChannel::Color7(_) => {
                (cmd::WRITE_RED_DATA, byte)
            }
        };
        bus.send_command(cmd)?;
        bus.send_data_repeated(byte, count)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::POWER_ON, &[0x00])?;
        bus.wait_busy(false)?;
        bus.send_command_with_data(cmd::DISPLAY_REFRESH, &[0x00])?;
        bus.wait_busy(false)
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::POWER_OFF, &[0x00])?;
        bus.wait_busy(false)
    }
}

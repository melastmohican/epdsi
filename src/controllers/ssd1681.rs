//! SSD1681 E-Paper Display Controller implementation.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// SSD1681 Command Definitions
pub mod cmd {
    /// Driver output control
    pub const DRIVER_CONTROL: u8 = 0x01;
    /// Gate driving voltage control
    pub const GATE_VOLTAGE: u8 = 0x03;
    /// Deep sleep mode entry
    pub const DEEP_SLEEP_MODE: u8 = 0x10;
    /// Data entry mode setting
    pub const DATA_ENTRY_MODE: u8 = 0x11;
    /// Software reset command
    pub const SW_RESET: u8 = 0x12;
    /// Temperature sensor control
    pub const TEMP_CONTROL: u8 = 0x18;
    /// Master activation command
    pub const MASTER_ACTIVATE: u8 = 0x20;
    /// Display update control 2
    pub const UPDATE_DISPLAY_CTRL2: u8 = 0x22;
    /// Write Black/White RAM data
    pub const WRITE_BW_DATA: u8 = 0x24;
    /// Write Red/Yellow RAM data
    pub const WRITE_RED_DATA: u8 = 0x26;
    /// Border waveform control
    pub const BORDER_WAVEFORM_CONTROL: u8 = 0x3C;
    /// Set RAM X address start/end position
    pub const SET_RAMXPOS: u8 = 0x44;
    /// Set RAM Y address start/end position
    pub const SET_RAMYPOS: u8 = 0x45;
    /// Set RAM X address counter
    pub const SET_RAMXCNT: u8 = 0x4E;
    /// Set RAM Y address counter
    pub const SET_RAMYCNT: u8 = 0x4F;
}

/// SSD1681 Controller IC driver configuration.
#[derive(Debug, Clone, Copy)]
pub struct Ssd1681Controller {
    width: u32,
    height: u32,
}

impl Ssd1681Controller {
    /// Creates a new SSD1681 controller configured for target dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Ssd1681Controller
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
        bus.hard_reset(delay, 10)?;
        bus.send_command(cmd::SW_RESET)?;
        bus.wait_busy(true)?;

        // Driver output control: setting display height
        let h_low = ((self.height - 1) & 0xFF) as u8;
        let h_high = (((self.height - 1) >> 8) & 0xFF) as u8;
        bus.send_command_with_data(cmd::DRIVER_CONTROL, &[h_low, h_high, 0x00])?;

        // Data Entry Mode: Increment X, Increment Y
        bus.send_command_with_data(cmd::DATA_ENTRY_MODE, &[0x03])?;

        // Set RAM Area to full display frame
        self.set_window(bus, 0, 0, self.width - 1, self.height - 1)?;
        self.set_cursor(bus, 0, 0)?;

        // Border Waveform Control
        bus.send_command_with_data(cmd::BORDER_WAVEFORM_CONTROL, &[0x05])?;

        // Internal Temperature Sensor Selection
        bus.send_command_with_data(cmd::TEMP_CONTROL, &[0x80])?;

        bus.wait_busy(true)?;
        Ok(())
    }

    fn set_window(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error> {
        let x_start_byte = (x_start / 8) as u8;
        let x_end_byte = (x_end / 8) as u8;

        bus.send_command_with_data(cmd::SET_RAMXPOS, &[x_start_byte, x_end_byte])?;
        bus.send_command_with_data(
            cmd::SET_RAMYPOS,
            &[
                (y_start & 0xFF) as u8,
                ((y_start >> 8) & 0xFF) as u8,
                (y_end & 0xFF) as u8,
                ((y_end >> 8) & 0xFF) as u8,
            ],
        )
    }

    fn set_cursor(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x: u32,
        y: u32,
    ) -> Result<(), Self::Error> {
        let x_byte = (x / 8) as u8;
        bus.send_command_with_data(cmd::SET_RAMXCNT, &[x_byte])?;
        bus.send_command_with_data(
            cmd::SET_RAMYCNT,
            &[(y & 0xFF) as u8, ((y >> 8) & 0xFF) as u8],
        )
    }

    fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        let cmd = match channel {
            ColorChannel::BlackWhite => cmd::WRITE_BW_DATA,
            ColorChannel::RedYellow
            | ColorChannel::Red
            | ColorChannel::Yellow
            | ColorChannel::Color7(_) => cmd::WRITE_RED_DATA,
        };
        bus.send_command_with_data(cmd, data)
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        let cmd = match channel {
            ColorChannel::BlackWhite => cmd::WRITE_BW_DATA,
            ColorChannel::RedYellow
            | ColorChannel::Red
            | ColorChannel::Yellow
            | ColorChannel::Color7(_) => cmd::WRITE_RED_DATA,
        };
        bus.send_command(cmd)?;
        bus.send_data_repeated(byte, count)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // Display Mode 1 update sequence
        bus.send_command_with_data(cmd::UPDATE_DISPLAY_CTRL2, &[0xF7])?;
        bus.send_command(cmd::MASTER_ACTIVATE)?;
        bus.wait_busy(true)
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::DEEP_SLEEP_MODE, &[0x01])
    }
}

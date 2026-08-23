//! SSD1677 E-Paper Display Controller implementation.
//!
//! Register set extends `Ssd1681Controller`'s (`0x01`/`0x11`/`0x3C`/`0x44`/`0x45`/`0x4E`/`0x4F`/`0x22`/`0x20`
//! window/refresh conventions) with a wider booster soft-start command and an explicit Display Update
//! Control 1 register. It also **widens the RAM X address**: the SSD1677 drives up to 960 source lines,
//! so `SET_RAMXPOS` takes four bytes (start and end as little-endian 16-bit values) and `SET_RAMXCNT`
//! two, and both are expressed in *pixels* rather than the byte indices SSD1680/SSD1681 use.
//! The target panel's gates are physically wired in reverse with no hardware gate-scan-direction bit,
//! so `set_window`/`set_cursor` flip the Y axis in software (Y-decrement data entry mode) rather than
//! relying on a register bit like other controllers in this crate.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// SSD1677 Command Definitions
pub mod cmd {
    /// Driver output control
    pub const DRIVER_CONTROL: u8 = 0x01;
    /// Booster soft-start control
    pub const BOOSTER_SOFT_START: u8 = 0x0C;
    /// Deep sleep mode entry
    pub const DEEP_SLEEP_MODE: u8 = 0x10;
    /// Data entry mode setting
    pub const DATA_ENTRY_MODE: u8 = 0x11;
    /// Software reset command
    pub const SW_RESET: u8 = 0x12;
    /// Temperature sensor control
    pub const TEMP_CONTROL: u8 = 0x18;
    /// Write to temperature register (direct override, used for fast-update waveform selection)
    pub const WRITE_TEMP_REG: u8 = 0x1A;
    /// Master activation command
    pub const MASTER_ACTIVATE: u8 = 0x20;
    /// Display update control 1 (RED bypass / single-chip selection)
    pub const DISPLAY_UPDATE_CTRL1: u8 = 0x21;
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

/// Display refresh operating mode for the SSD1677 controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Ssd1677RefreshMode {
    /// Full display update using the panel's OTP/full waveform LUT (`UPDATE_DISPLAY_CTRL2 = 0xF7`).
    #[default]
    Full,
    /// Full display update forced to a faster waveform via a direct temperature-register override
    /// (`WRITE_TEMP_REG = 0x5A`, `UPDATE_DISPLAY_CTRL2 = 0xD7`).
    FastFull,
    /// Partial display update using the controller's built-in fast LUT (`UPDATE_DISPLAY_CTRL2 = 0xFC`).
    Partial,
}

/// SSD1677 Controller IC driver configuration.
#[derive(Debug, Clone, Copy)]
pub struct Ssd1677Controller {
    width: u32,
    height: u32,
    refresh_mode: Ssd1677RefreshMode,
}

impl Ssd1677Controller {
    /// Creates a new SSD1677 controller configured for target dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            refresh_mode: Ssd1677RefreshMode::default(),
        }
    }

    /// Sets display refresh operating mode (builder method).
    pub fn with_refresh_mode(mut self, mode: Ssd1677RefreshMode) -> Self {
        self.refresh_mode = mode;
        self
    }

    /// Sets display refresh operating mode.
    pub fn set_refresh_mode(&mut self, mode: Ssd1677RefreshMode) {
        self.refresh_mode = mode;
    }

    /// Returns current display refresh mode.
    pub fn refresh_mode(&self) -> Ssd1677RefreshMode {
        self.refresh_mode
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Ssd1677Controller
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
        delay.delay_ms(10);

        // Internal Temperature Sensor Selection
        bus.send_command_with_data(cmd::TEMP_CONTROL, &[0x80])?;

        // Booster Soft-Start Control (wider payload than SSD1680/SSD1681)
        bus.send_command_with_data(cmd::BOOSTER_SOFT_START, &[0xAE, 0xC7, 0xC3, 0xC0, 0x80])?;

        // Driver output control: setting display height
        let h_low = ((self.height - 1) & 0xFF) as u8;
        let h_high = (((self.height - 1) >> 8) & 0xFF) as u8;
        bus.send_command_with_data(cmd::DRIVER_CONTROL, &[h_low, h_high, 0x02])?;

        // Border Waveform Control
        bus.send_command_with_data(cmd::BORDER_WAVEFORM_CONTROL, &[0x01])?;

        // Set RAM Area to full display frame. `set_window` also asserts the Increment-X /
        // Decrement-Y data entry mode the reversed gates require.
        self.set_window(bus, 0, 0, self.width - 1, self.height - 1)?;
        self.set_cursor(bus, 0, 0)?;

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
        // Unlike SSD1680/SSD1681, which address RAM X by *byte index* in a single register byte,
        // the SSD1677 drives up to 960 source lines and takes a 2-byte, *pixel*-valued X address
        // for both the start and the end of the window. Round the span out to whole bytes.
        let xs = x_start & !7;
        let xe = x_end | 7;

        // Y is reversed in RAM: the panel gates are physically wired in reverse, so the visible-space
        // window (y_start..=y_end) maps to a decrementing RAM Y range starting at the "high" end.
        let h = y_end - y_start + 1;
        let yy = self.height - y_start - h;
        let yy_end = self.height - y_start - 1;

        // Re-assert Y-decrement data entry alongside every window change, matching GxEPD2.
        bus.send_command_with_data(cmd::DATA_ENTRY_MODE, &[0x01])?;
        bus.send_command_with_data(
            cmd::SET_RAMXPOS,
            &[
                (xs & 0xFF) as u8,
                ((xs >> 8) & 0xFF) as u8,
                (xe & 0xFF) as u8,
                ((xe >> 8) & 0xFF) as u8,
            ],
        )?;
        bus.send_command_with_data(
            cmd::SET_RAMYPOS,
            &[
                (yy_end & 0xFF) as u8,
                ((yy_end >> 8) & 0xFF) as u8,
                (yy & 0xFF) as u8,
                ((yy >> 8) & 0xFF) as u8,
            ],
        )
    }

    fn set_cursor(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x: u32,
        y: u32,
    ) -> Result<(), Self::Error> {
        // 2-byte, pixel-valued X counter to match `SET_RAMXPOS`; see `set_window`.
        let xx = x & !7;
        let yy_cursor = self.height - 1 - y;
        bus.send_command_with_data(
            cmd::SET_RAMXCNT,
            &[(xx & 0xFF) as u8, ((xx >> 8) & 0xFF) as u8],
        )?;
        bus.send_command_with_data(
            cmd::SET_RAMYCNT,
            &[(yy_cursor & 0xFF) as u8, ((yy_cursor >> 8) & 0xFF) as u8],
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
        let bypass = match self.refresh_mode {
            Ssd1677RefreshMode::Full | Ssd1677RefreshMode::FastFull => [0x40, 0x00],
            Ssd1677RefreshMode::Partial => [0x00, 0x00],
        };
        bus.send_command_with_data(cmd::DISPLAY_UPDATE_CTRL1, &bypass)?;

        if self.refresh_mode == Ssd1677RefreshMode::FastFull {
            bus.send_command_with_data(cmd::WRITE_TEMP_REG, &[0x5A])?;
        }

        let mode_byte = match self.refresh_mode {
            Ssd1677RefreshMode::Full => 0xF7,
            Ssd1677RefreshMode::FastFull => 0xD7,
            Ssd1677RefreshMode::Partial => 0xFC,
        };
        bus.send_command_with_data(cmd::UPDATE_DISPLAY_CTRL2, &[mode_byte])?;
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

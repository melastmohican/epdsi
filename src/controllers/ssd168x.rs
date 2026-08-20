//! SSD168x (SSD1680 & SSD1681) E-Paper Display Controller implementation.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// SSD168x Command Definitions
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
    /// Display update control 1
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

/// SSD168x IC variant family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ssd168xVariant {
    /// SSD1680 IC variant (supports 176×296 RAM space, uses explicit power-stage commands 0xE0/0x83).
    #[default]
    Ssd1680,
    /// SSD1681 IC variant (supports 200×200 RAM space, uses direct display update trigger 0xF7/0xFC).
    Ssd1681,
}

/// Display refresh operating mode for SSD168x controllers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Ssd168xRefreshMode {
    /// Full display update using the panel's OTP/full waveform LUT (`UPDATE_DISPLAY_CTRL2 = 0xF7`).
    #[default]
    Full,
    /// Partial display update using the controller's built-in fast LUT (`UPDATE_DISPLAY_CTRL2 = 0xFC`).
    Partial,
}

/// Generic SSD168x (SSD1680 / SSD1681) Controller IC driver implementation.
#[derive(Debug, Clone, Copy)]
pub struct Ssd168xController {
    width: u32,
    height: u32,
    variant: Ssd168xVariant,
    refresh_mode: Ssd168xRefreshMode,
}

/// Dedicated controller for SSD1680 IC (176×296 RAM, power stage 0xE0/0x83).
#[derive(Debug, Clone, Copy)]
pub struct Ssd1680Controller {
    inner: Ssd168xController,
}

/// Dedicated controller for SSD1681 IC (200×200 RAM, direct trigger 0xF7/0xFC).
#[derive(Debug, Clone, Copy)]
pub struct Ssd1681Controller {
    inner: Ssd168xController,
}

/// Backwards-compatible type alias for SSD1680 refresh mode.
pub type Ssd1680RefreshMode = Ssd168xRefreshMode;
/// Backwards-compatible type alias for SSD1681 refresh mode.
pub type Ssd1681RefreshMode = Ssd168xRefreshMode;

impl Ssd1680Controller {
    /// Creates a new SSD1680 controller instance configured for target display dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            inner: Ssd168xController::new_ssd1680(width, height),
        }
    }

    /// Sets display refresh operating mode (builder method).
    pub fn with_refresh_mode(mut self, mode: Ssd168xRefreshMode) -> Self {
        self.inner = self.inner.with_refresh_mode(mode);
        self
    }

    /// Sets display refresh operating mode.
    pub fn set_refresh_mode(&mut self, mode: Ssd168xRefreshMode) {
        self.inner.set_refresh_mode(mode);
    }

    /// Returns current display refresh mode.
    pub fn refresh_mode(&self) -> Ssd168xRefreshMode {
        self.inner.refresh_mode()
    }

    /// Access underlying generic controller.
    pub fn into_inner(self) -> Ssd168xController {
        self.inner
    }
}

impl Ssd1681Controller {
    /// Creates a new SSD1681 controller instance configured for target display dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            inner: Ssd168xController::new_ssd1681(width, height),
        }
    }

    /// Sets display refresh operating mode (builder method).
    pub fn with_refresh_mode(mut self, mode: Ssd168xRefreshMode) -> Self {
        self.inner = self.inner.with_refresh_mode(mode);
        self
    }

    /// Sets display refresh operating mode.
    pub fn set_refresh_mode(&mut self, mode: Ssd168xRefreshMode) {
        self.inner.set_refresh_mode(mode);
    }

    /// Returns current display refresh mode.
    pub fn refresh_mode(&self) -> Ssd168xRefreshMode {
        self.inner.refresh_mode()
    }

    /// Access underlying generic controller.
    pub fn into_inner(self) -> Ssd168xController {
        self.inner
    }
}

impl Ssd168xController {
    /// Creates a new SSD1680 controller instance configured for target display dimensions.
    pub fn new_ssd1680(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            variant: Ssd168xVariant::Ssd1680,
            refresh_mode: Ssd168xRefreshMode::default(),
        }
    }

    /// Creates a new SSD1681 controller instance configured for target display dimensions.
    pub fn new_ssd1681(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            variant: Ssd168xVariant::Ssd1681,
            refresh_mode: Ssd168xRefreshMode::default(),
        }
    }

    /// Creates a new SSD168x controller with target dimensions and variant.
    pub fn new(width: u32, height: u32, variant: Ssd168xVariant) -> Self {
        Self {
            width,
            height,
            variant,
            refresh_mode: Ssd168xRefreshMode::default(),
        }
    }

    /// Sets driver IC variant (builder method).
    pub fn with_variant(mut self, variant: Ssd168xVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets driver IC variant.
    pub fn set_variant(&mut self, variant: Ssd168xVariant) {
        self.variant = variant;
    }

    /// Returns current driver IC variant.
    pub fn variant(&self) -> Ssd168xVariant {
        self.variant
    }

    /// Sets display refresh operating mode (builder method).
    pub fn with_refresh_mode(mut self, mode: Ssd168xRefreshMode) -> Self {
        self.refresh_mode = mode;
        self
    }

    /// Sets display refresh operating mode.
    pub fn set_refresh_mode(&mut self, mode: Ssd168xRefreshMode) {
        self.refresh_mode = mode;
    }

    /// Returns current display refresh mode.
    pub fn refresh_mode(&self) -> Ssd168xRefreshMode {
        self.refresh_mode
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Ssd1680Controller
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
        self.inner.init_sequence(bus, delay)
    }

    fn set_window(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error> {
        self.inner.set_window(bus, x_start, y_start, x_end, y_end)
    }

    fn set_cursor(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x: u32,
        y: u32,
    ) -> Result<(), Self::Error> {
        self.inner.set_cursor(bus, x, y)
    }

    fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.inner.write_frame(bus, channel, data)
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        self.inner.write_frame_pattern(bus, channel, byte, count)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.trigger_refresh(bus, delay)
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.sleep(bus, delay)
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
        self.inner.init_sequence(bus, delay)
    }

    fn set_window(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error> {
        self.inner.set_window(bus, x_start, y_start, x_end, y_end)
    }

    fn set_cursor(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x: u32,
        y: u32,
    ) -> Result<(), Self::Error> {
        self.inner.set_cursor(bus, x, y)
    }

    fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.inner.write_frame(bus, channel, data)
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        self.inner.write_frame_pattern(bus, channel, byte, count)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.trigger_refresh(bus, delay)
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.sleep(bus, delay)
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Ssd168xController
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
        delay.delay_ms(1);
        bus.wait_busy_with_delay(delay, true)?;

        // Driver output control: setting display height
        let h_low = ((self.height - 1) & 0xFF) as u8;
        let h_high = (((self.height - 1) >> 8) & 0xFF) as u8;
        bus.send_command_with_data(cmd::DRIVER_CONTROL, &[h_low, h_high, 0x00])?;

        // Border Waveform Control
        bus.send_command_with_data(cmd::BORDER_WAVEFORM_CONTROL, &[0x05])?;

        // SSD1680 specific: Display Update Control 1 (RAM content option / source output mode)
        if self.variant == Ssd168xVariant::Ssd1680 {
            bus.send_command_with_data(cmd::DISPLAY_UPDATE_CTRL1, &[0x00, 0x80])?;
        }

        // Internal Temperature Sensor Selection
        bus.send_command_with_data(cmd::TEMP_CONTROL, &[0x80])?;

        // Data Entry Mode: Increment X, Increment Y
        bus.send_command_with_data(cmd::DATA_ENTRY_MODE, &[0x03])?;

        // Set RAM Area to full display frame
        self.set_window(bus, 0, 0, self.width - 1, self.height - 1)?;
        self.set_cursor(bus, 0, 0)?;

        delay.delay_ms(1);
        bus.wait_busy_with_delay(delay, true)?;
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
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        let mode_byte = match self.refresh_mode {
            Ssd168xRefreshMode::Full => 0xF7,
            Ssd168xRefreshMode::Partial => 0xFC,
        };

        match self.variant {
            Ssd168xVariant::Ssd1680 => {
                // Power on sequence
                bus.send_command_with_data(cmd::UPDATE_DISPLAY_CTRL2, &[0xE0])?;
                bus.send_command(cmd::MASTER_ACTIVATE)?;
                delay.delay_ms(1);
                bus.wait_busy_with_delay(delay, true)?;

                // Display update sequence (Full: OTP LUT, Partial: built-in fast LUT)
                bus.send_command_with_data(cmd::UPDATE_DISPLAY_CTRL2, &[mode_byte])?;
                bus.send_command(cmd::MASTER_ACTIVATE)?;
                delay.delay_ms(1);
                bus.wait_busy_with_delay(delay, true)?;

                // Power off sequence
                bus.send_command_with_data(cmd::UPDATE_DISPLAY_CTRL2, &[0x83])?;
                bus.send_command(cmd::MASTER_ACTIVATE)?;
                delay.delay_ms(1);
                bus.wait_busy_with_delay(delay, true)
            }
            Ssd168xVariant::Ssd1681 => {
                bus.send_command_with_data(cmd::UPDATE_DISPLAY_CTRL2, &[mode_byte])?;
                bus.send_command(cmd::MASTER_ACTIVATE)?;
                delay.delay_ms(1);
                bus.wait_busy_with_delay(delay, true)
            }
        }
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::DEEP_SLEEP_MODE, &[0x01])
    }
}

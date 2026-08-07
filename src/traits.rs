//! Core trait definitions for E-Paper Display Serial Interface (`epdsi`).
//!
//! Decouples the physical display panel specs (`EpdPanel`) from the driver IC logic (`EpdController`).

use embedded_hal::delay::DelayNs;

/// Color operation modes supported by E-Paper displays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Monochrome (1-bit Black and White)
    BlackWhite,
    /// Tri-Color (Black, White, and Red or Yellow)
    TriColor,
    /// 7-Color Advanced Color ePaper (ACeP)
    SevenColor,
}

/// Identifies the frame buffer/RAM color channel being targeted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorChannel {
    /// Primary Black/White image channel
    BlackWhite,
    /// Secondary Red/Yellow image channel (for Tri-Color panels)
    RedYellow,
    /// Multi-color channel indexing for 7-color displays
    Color7(u8),
}

/// Trait representing physical display panel parameters and configuration overrides.
pub trait EpdPanel {
    /// Physical width of the panel in pixels.
    const WIDTH: u32;

    /// Physical height of the panel in pixels.
    const HEIGHT: u32;

    /// Color capability mode of the panel.
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;

    /// Optional VCOM voltage configuration override byte.
    fn vcom(&self) -> Option<u8> {
        None
    }

    /// Optional custom embedded Look-Up Table (LUT) for driving waveforms.
    fn custom_lut(&self) -> Option<&'static [u8]> {
        None
    }

    /// Optional Gate Driving Voltage override parameter.
    fn gate_voltage(&self) -> Option<u8> {
        None
    }
}

/// Trait encapsulating driver IC command sets, register sequences, and refresh triggers.
pub trait EpdController<BUS> {
    /// Error type emitted by controller operations or underlying bus.
    type Error;

    /// Executes the hardware/software initialization sequence for the driver IC.
    fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut BUS,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error>;

    /// Sets active display RAM window coordinates.
    fn set_window(
        &mut self,
        bus: &mut BUS,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error>;

    /// Sets RAM address cursor position.
    fn set_cursor(&mut self, bus: &mut BUS, x: u32, y: u32) -> Result<(), Self::Error>;

    /// Writes raw slice data to targeted color channel RAM.
    fn write_frame(
        &mut self,
        bus: &mut BUS,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error>;

    /// Writes a repeating byte pattern to display RAM (useful for clearing frames).
    fn write_frame_pattern(
        &mut self,
        bus: &mut BUS,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error>;

    /// Triggers display update refresh and waits until busy signal clears.
    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut BUS,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error>;

    /// Puts the controller IC into deep sleep power saving mode.
    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut BUS,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error>;
}

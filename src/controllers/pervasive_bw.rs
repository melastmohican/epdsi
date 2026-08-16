//! Pervasive Displays BW/BWR (Black/White, Black/White/Red or Yellow) E-Paper Display Controller
//! implementation (DriverC/DriverF COG family). For the Spectra-4/BWRY family, see
//! [`crate::controllers::pervasive_bwry::PervasiveBwryController`].

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
    /// VCOM and Data Interval Setting (CDI)
    pub const VCOM_INTERVAL: u8 = 0x50;
    /// Active Temperature sensor selection
    pub const ACTIVE_TEMP: u8 = 0xE0;
    /// Input Temperature value selection
    pub const INPUT_TEMP: u8 = 0xE5;
}

// Register configuration constants
const REG_DATA_SOFT_RESET: &[u8] = &[0x0E];
const REG_DATA_ACTIVE_TEMP: &[u8] = &[0x02];

/// Display refresh operating mode for Pervasive Displays controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PervasiveRefreshMode {
    /// Standard full screen update mode.
    #[default]
    Normal,
    /// Embedded fast differential update mode.
    Fast,
}

/// Pervasive COG driver IC variant family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PervasiveDriverVariant {
    /// COG Driver C / E / D / 9 (e.g. E2266KS0C1 / EPD_266_KS_0C). Uses Panel Setting Register (PSR 0x00).
    #[default]
    DriverC,
    /// COG Driver F (e.g. E2290KS0F1 / EPD_290_KS_0F). Uses registers 0x4D (0x55) and 0xE9 (0x02) instead of PSR.
    DriverF,
}

/// Pervasive Displays BW/BWR COG Controller IC driver implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PervasiveBwController {
    width: u32,
    height: u32,
    temperature_c: i8,
    refresh_mode: PervasiveRefreshMode,
    driver_variant: PervasiveDriverVariant,
    psr: [u8; 2],
    auto_clear_secondary: bool,
}

impl Default for PervasiveBwController {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            temperature_c: 25,
            refresh_mode: PervasiveRefreshMode::Normal,
            driver_variant: PervasiveDriverVariant::DriverC,
            psr: [0xCF, 0x8D],
            auto_clear_secondary: true,
        }
    }
}

impl PervasiveBwController {
    /// Creates a new Pervasive Displays BW/BWR controller instance with target resolution.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            ..Default::default()
        }
    }

    /// Sets operating ambient temperature in Celsius (builder method).
    pub fn with_temperature(mut self, temperature_c: i8) -> Self {
        self.temperature_c = temperature_c;
        self
    }

    /// Sets ambient temperature in Celsius.
    pub fn set_temperature(&mut self, temperature_c: i8) {
        self.temperature_c = temperature_c;
    }

    /// Returns current operating ambient temperature in Celsius.
    pub fn temperature(&self) -> i8 {
        self.temperature_c
    }

    /// Sets display refresh operating mode (builder method).
    pub fn with_refresh_mode(mut self, mode: PervasiveRefreshMode) -> Self {
        self.refresh_mode = mode;
        self
    }

    /// Sets display refresh operating mode.
    pub fn set_refresh_mode(&mut self, mode: PervasiveRefreshMode) {
        self.refresh_mode = mode;
    }

    /// Returns current display refresh mode.
    pub fn refresh_mode(&self) -> PervasiveRefreshMode {
        self.refresh_mode
    }

    /// Sets driver IC variant (builder method).
    pub fn with_driver_variant(mut self, variant: PervasiveDriverVariant) -> Self {
        self.driver_variant = variant;
        self
    }

    /// Sets driver IC variant.
    pub fn set_driver_variant(&mut self, variant: PervasiveDriverVariant) {
        self.driver_variant = variant;
    }

    /// Returns current driver IC variant.
    pub fn driver_variant(&self) -> PervasiveDriverVariant {
        self.driver_variant
    }

    /// Configures Panel Setting Register (PSR) calibration parameters (builder method).
    pub fn with_psr(mut self, psr: [u8; 2]) -> Self {
        self.psr = psr;
        self
    }

    /// Configures whether normal updates automatically clear secondary RAM buffer (`WRITE_RED_DATA`) with `0x00` (builder method).
    pub fn with_auto_clear_secondary(mut self, auto_clear: bool) -> Self {
        self.auto_clear_secondary = auto_clear;
        self
    }

    /// Writes previous and current frame data for embedded fast differential update mode.
    #[allow(clippy::type_complexity)]
    pub fn write_fast_frame<SPI, DC, RST, BUSY>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        previous_frame: &[u8],
        current_frame: &[u8],
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        // VCOM & data interval setting prior to frame write
        bus.send_command_with_data(cmd::VCOM_INTERVAL, &[0x27])?;

        // First frame (0x10, DTM1) receives OLD / previous image (inverted for display logic)
        bus.send_command(cmd::WRITE_BW_DATA)?;
        let mut buf = [0u8; 64];
        for chunk in previous_frame.chunks(64) {
            for (i, &b) in chunk.iter().enumerate() {
                buf[i] = !b;
            }
            bus.send_data(&buf[..chunk.len()])?;
        }

        // Second frame (0x13, DTM2) receives NEW / current image (inverted for display logic)
        bus.send_command(cmd::WRITE_RED_DATA)?;
        for chunk in current_frame.chunks(64) {
            for (i, &b) in chunk.iter().enumerate() {
                buf[i] = !b;
            }
            bus.send_data(&buf[..chunk.len()])?;
        }

        // Reset VCOM & data interval setting post frame write
        bus.send_command_with_data(cmd::VCOM_INTERVAL, &[0x07])?;

        Ok(())
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>>
    for PervasiveBwController
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
        bus.wait_busy_with_delay(delay, false)?;

        // Calculate work settings based on update mode
        let (temp_val, psr_val) = match self.refresh_mode {
            PervasiveRefreshMode::Normal => (self.temperature_c as u8, self.psr),
            PervasiveRefreshMode::Fast => (
                (self.temperature_c as u8) | 0x40,
                [self.psr[0] | 0x10, self.psr[1] | 0x02],
            ),
        };

        // Soft reset command
        bus.send_command_with_data(cmd::PSR, REG_DATA_SOFT_RESET)?;
        bus.wait_busy_with_delay(delay, false)?;

        // Temperature calibration
        bus.send_command_with_data(cmd::INPUT_TEMP, &[temp_val])?;
        bus.send_command_with_data(cmd::ACTIVE_TEMP, REG_DATA_ACTIVE_TEMP)?;

        // Driver variant configuration
        match self.driver_variant {
            PervasiveDriverVariant::DriverC => {
                bus.send_command_with_data(cmd::PSR, &psr_val)?;
            }
            PervasiveDriverVariant::DriverF => {
                bus.send_command_with_data(0x4D, &[0x55])?;
                bus.send_command_with_data(0xE9, &[0x02])?;
            }
        }

        // Fast update VCOM & Data Interval Setting
        if self.refresh_mode == PervasiveRefreshMode::Fast {
            bus.send_command_with_data(cmd::VCOM_INTERVAL, &[0x07])?;
        }

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

                if self.refresh_mode == PervasiveRefreshMode::Normal && self.auto_clear_secondary {
                    bus.send_command(cmd::WRITE_RED_DATA)?;
                    bus.send_data_repeated(0x00, data.len())?;
                }
                Ok(())
            }
            ColorChannel::RedYellow
            | ColorChannel::Red
            | ColorChannel::Yellow
            | ColorChannel::Color7(_) => bus.send_command_with_data(cmd::WRITE_RED_DATA, data),
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
            ColorChannel::RedYellow
            | ColorChannel::Red
            | ColorChannel::Yellow
            | ColorChannel::Color7(_) => (cmd::WRITE_RED_DATA, byte),
        };
        bus.send_command(cmd)?;
        bus.send_data_repeated(byte, count)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.wait_busy_with_delay(delay, false)?;
        bus.send_command(cmd::POWER_ON)?;
        bus.wait_busy_with_delay(delay, false)?;
        bus.send_command(cmd::DISPLAY_REFRESH)?;
        bus.wait_busy_with_delay(delay, false)
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::POWER_OFF)?;
        bus.wait_busy_with_delay(delay, false)
    }
}

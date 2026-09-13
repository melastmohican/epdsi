//! JD7966x (JD79660AA & JD79661AA) E-Paper Display Controller implementation.
//!
//! Both ICs share an identical SPI command-register table (confirmed against the JD79660A
//! v1.0.3 and JD79661AA v1.0.4 datasheets directly — same addresses, bit fields, and lengths
//! for PSR, PWR, POF, PON, BTST, DSLP, DTM, DRF, PLL, CDI, TRES), differing only in which
//! registers are written during init and what values go in them — the same relationship
//! [`Ssd168xController`](crate::controllers::Ssd168xController)/
//! [`Ssd168xVariant`](crate::controllers::Ssd168xVariant) models for SSD1680/SSD1681.
//! [`Jd7966xController::init_sequence`] is the only method that branches on the variant; every
//! other method (`write_frame`, `write_frame_pattern`, `set_window`, `set_cursor`,
//! `trigger_refresh`, `sleep`) is byte-identical between the two ICs and written once.
//!
//! `trigger_refresh` (`DISPLAY_REFRESH`, `0x12`) and `sleep` (`POWER_OFF`, `0x02`) both send a
//! trailing `0x00` data byte on both variants: both datasheets document that byte as mandatory
//! (`R02H`/`R12H`, "1 data byte, default 00h" — JD79661AA v1.0.4 §8.2.3/§8.2.9), and Adafruit's
//! own `Adafruit_JD79661.cpp` (`update()`/`powerDown()`) sends it explicitly. Both commands used
//! to be sent bare on the JD79661 side of this file — a latent bug caught only by reading the
//! datasheet directly, not by the vendor C++ references agreeing with each other.
//!
//! `0x4D`, `0xE7`, `0xE3`, `0xB4` and `0xB5` are vendor registers **not documented in either
//! public datasheet** — the same class of undocumented-but-real register as the SSD1680 `0x3F`
//! "Option for LUT end" precedent — sourced only from the Waveshare/Adafruit reference drivers.
//!
//! JD79660AA also has a fast-update init variant in both vendor references (`Init_Fast()` /
//! `_use_fast_update`), but they disagree on register-write ordering relative to `PowerOn` and
//! the vendor's own demo defaults to the plain path, so it is not implemented here.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::spi::SpiDevice;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::spi::SpiDevice;

use embedded_hal::digital::{InputPin, OutputPin};
// `Wait` has no blocking equivalent, so alias it to `InputPin` under `blocking` — the bound
// `BUSY: InputPin + Wait` then reads identically in both modes (redundantly `InputPin + InputPin`
// when aliased, which Rust allows) with no per-identifier macro mapping needed.
#[cfg(feature = "blocking")]
use embedded_hal::digital::InputPin as Wait;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// JD7966x Command Definitions
pub mod cmd {
    /// Panel Setting command
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power Setting / Software Reset command (JD79661 only)
    pub const POWER_SETTING: u8 = 0x01;
    /// Power OFF command
    pub const POWER_OFF: u8 = 0x02;
    /// Power Offset command (JD79661 only)
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
    /// PLL Control register. Named `VCOM_CONTROL` in an earlier revision of this file — both
    /// datasheets name `0x30` "PLL Control"; the real VCOM-DC register is `0x82`, unused by
    /// either variant here (both rely on OTP/POR default VCOM).
    pub const PLL_CONTROL: u8 = 0x30;
    /// Vendor Magic Key. Undocumented in either public datasheet.
    pub const MAGIC_KEY: u8 = 0x4D;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// TCON Setting (JD79661 only)
    pub const TCON: u8 = 0x60;
    /// Resolution Setting
    pub const RESOLUTION: u8 = 0x61;
}

/// JD7966x IC variant family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Jd7966xVariant {
    /// JD79660AA — plain (non-fast) init only. Smaller register subset than
    /// [`Jd7966xVariant::Jd79661`]: no `POWER_SETTING`/`POWER_OFFSET`/`TCON`, and none of
    /// `0xE7`/`0xE3`/`0xB4`/`0xB5`. Drives `GDEM0154F51H`.
    #[default]
    Jd79660,
    /// JD79661AA — drives `ZJY122250_0213AJH_E5` / `GDEY0213F51`.
    Jd79661,
}

/// Generic JD7966x (JD79660AA / JD79661AA) Controller IC driver implementation.
#[derive(Debug, Clone, Copy)]
pub struct Jd7966xController {
    width: u32,
    height: u32,
    variant: Jd7966xVariant,
}

/// Dedicated controller for the JD79660AA IC (`GDEM0154F51H`).
#[derive(Debug, Clone, Copy)]
pub struct Jd79660Controller {
    inner: Jd7966xController,
}

/// Dedicated controller for the JD79661AA IC (`ZJY122250_0213AJH_E5` / `GDEY0213F51`).
#[derive(Debug, Clone, Copy)]
pub struct Jd79661Controller {
    inner: Jd7966xController,
}

impl Jd79660Controller {
    /// Creates a new JD79660AA controller instance configured for target display dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            inner: Jd7966xController::new_jd79660(width, height),
        }
    }
}

impl Jd79661Controller {
    /// Creates a new JD79661AA controller instance configured for target display dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            inner: Jd7966xController::new_jd79661(width, height),
        }
    }
}

impl Jd7966xController {
    /// Creates a new JD79660AA controller instance configured for target display dimensions.
    pub fn new_jd79660(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            variant: Jd7966xVariant::Jd79660,
        }
    }

    /// Creates a new JD79661AA controller instance configured for target display dimensions.
    pub fn new_jd79661(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            variant: Jd7966xVariant::Jd79661,
        }
    }

    /// Creates a new JD7966x controller with target dimensions and variant.
    pub fn new(width: u32, height: u32, variant: Jd7966xVariant) -> Self {
        Self {
            width,
            height,
            variant,
        }
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Jd7966xController
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin + Wait,
{
    type Error = EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;

    async fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // Hardware reset sequence. Busy is active-low on both variants: busy when LOW.
        bus.hard_reset(delay, 10).await?;
        bus.wait_busy(false).await?;

        match self.variant {
            Jd7966xVariant::Jd79660 => {
                // Vendor unlock key
                bus.send_command_with_data(cmd::MAGIC_KEY, &[0x78]).await?;

                // Panel Setting
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x0F, 0x29])
                    .await?;

                // Booster Soft Start
                bus.send_command_with_data(
                    cmd::BOOSTER_SOFT_START,
                    &[0x0D, 0x12, 0x30, 0x20, 0x19, 0x2A, 0x22],
                )
                .await?;

                // CDI (VCOM and Data Interval Setting)
                bus.send_command_with_data(cmd::CDI, &[0x37]).await?;

                // Resolution setting — 200x200 is already 8-pixel aligned, no rounding needed.
                let w_high = ((self.width >> 8) & 0xFF) as u8;
                let w_low = (self.width & 0xFF) as u8;
                let h_high = ((self.height >> 8) & 0xFF) as u8;
                let h_low = (self.height & 0xFF) as u8;
                bus.send_command_with_data(cmd::RESOLUTION, &[w_high, w_low, h_high, h_low])
                    .await?;

                // Undocumented in either public datasheet; sourced from Waveshare + GxEPD2 only.
                bus.send_command_with_data(0xE9, &[0x01]).await?;
                bus.send_command_with_data(cmd::PLL_CONTROL, &[0x08])
                    .await?;
            }
            Jd7966xVariant::Jd79661 => {
                bus.send_command_with_data(cmd::POWER_SETTING, &[]).await?;
                bus.wait_busy(false).await?;

                // Vendor unlock key
                bus.send_command_with_data(cmd::MAGIC_KEY, &[0x78]).await?;

                // Panel Setting (128x250)
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x8F, 0x29])
                    .await?;

                // Power setting
                bus.send_command_with_data(cmd::POWER_SETTING, &[0x07, 0x00])
                    .await?;

                // Power offset
                bus.send_command_with_data(cmd::POWER_OFFSET, &[0x10, 0x54, 0x44])
                    .await?;

                // Booster Soft Start
                bus.send_command_with_data(
                    cmd::BOOSTER_SOFT_START,
                    &[0x05, 0x00, 0x3F, 0x0A, 0x25, 0x12, 0x1A],
                )
                .await?;

                // CDI (VCOM and Data Interval Setting)
                bus.send_command_with_data(cmd::CDI, &[0x37]).await?;

                // TCON
                bus.send_command_with_data(cmd::TCON, &[0x02, 0x02, 0x02])
                    .await?;

                // Resolution setting (Align RAM width to 8-pixel byte boundary, e.g. 128 x 250)
                let ram_width = self.width.div_ceil(8) * 8;
                let w_high = ((ram_width >> 8) & 0xFF) as u8;
                let w_low = (ram_width & 0xFF) as u8;
                let h_high = ((self.height >> 8) & 0xFF) as u8;
                let h_low = (self.height & 0xFF) as u8;
                bus.send_command_with_data(cmd::RESOLUTION, &[w_high, w_low, h_high, h_low])
                    .await?;

                // Undocumented in either public datasheet; sourced from Adafruit's reference only.
                bus.send_command_with_data(0xE7, &[0x1C]).await?;
                bus.send_command_with_data(0xE3, &[0x22]).await?;
                bus.send_command_with_data(0xB4, &[0xD0]).await?;
                bus.send_command_with_data(0xB5, &[0x03]).await?;
                bus.send_command_with_data(0xE9, &[0x01]).await?;
                bus.send_command_with_data(cmd::PLL_CONTROL, &[0x08])
                    .await?;
            }
        }

        // Power ON and wait until ready
        bus.send_command(cmd::POWER_ON).await?;
        bus.wait_busy(false).await?;

        Ok(())
    }

    async fn set_window(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x_start: u32,
        _y_start: u32,
        _x_end: u32,
        _y_end: u32,
    ) -> Result<(), Self::Error> {
        // Both variants use full frame buffer streaming by default.
        Ok(())
    }

    async fn set_cursor(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x: u32,
        _y: u32,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::DATA_START_TRANSMISSION, data)
            .await
    }

    async fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::DATA_START_TRANSMISSION).await?;
        bus.send_data_repeated(byte, count).await
    }

    async fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::DISPLAY_REFRESH, &[0x00])
            .await?;
        bus.wait_busy(false).await
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::POWER_OFF, &[0x00]).await?;
        bus.wait_busy(false).await?;
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5]).await
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Jd79660Controller
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin + Wait,
{
    type Error = EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;

    async fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.init_sequence(bus, delay).await
    }

    async fn set_window(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error> {
        self.inner
            .set_window(bus, x_start, y_start, x_end, y_end)
            .await
    }

    async fn set_cursor(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x: u32,
        y: u32,
    ) -> Result<(), Self::Error> {
        self.inner.set_cursor(bus, x, y).await
    }

    async fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.inner.write_frame(bus, channel, data).await
    }

    async fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        self.inner
            .write_frame_pattern(bus, channel, byte, count)
            .await
    }

    async fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.trigger_refresh(bus, delay).await
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.sleep(bus, delay).await
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Jd79661Controller
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin + Wait,
{
    type Error = EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;

    async fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.init_sequence(bus, delay).await
    }

    async fn set_window(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error> {
        self.inner
            .set_window(bus, x_start, y_start, x_end, y_end)
            .await
    }

    async fn set_cursor(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x: u32,
        y: u32,
    ) -> Result<(), Self::Error> {
        self.inner.set_cursor(bus, x, y).await
    }

    async fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        self.inner.write_frame(bus, channel, data).await
    }

    async fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        self.inner
            .write_frame_pattern(bus, channel, byte, count)
            .await
    }

    async fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.trigger_refresh(bus, delay).await
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        self.inner.sleep(bus, delay).await
    }
}

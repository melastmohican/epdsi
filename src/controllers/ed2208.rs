//! ED2208 E-Paper Display Controller implementation.
//!
//! Driver IC for 4 bpp palette e-paper panels — ACeP 7-colour and E Ink Spectra 6
//! (such as Good Display GDEP073E01, which is a Spectra 6 / E6 panel).
//!
//! # Where the "ED2208" designation comes from
//!
//! GxEPD2, the reference this implementation is audited against, names its classes
//! after panels (`GxEPD2_730c_GDEP073E01`) and never names the controller — so the
//! part number is not recoverable from it.
//!
//! The name comes from the hardware. Zephyr's mainline board port for the Seeed
//! reTerminal E1002, which carries a GDEP073E01, declares the display controller as
//! **ED2208-GCA** ("E-Ink ED2208-GCA Display Controller") behind the devicetree
//! compatible string `eink,ed2208-gca`.
//!
//! `ED2208` is an E Ink part family with suffixed variants: `-GCA` drives the 7.3"
//! 800x480 panel targeted here, while `-NCA` is the 13.3" 1600x1200 part. This
//! controller implements the `-GCA` command set.
//!
//! See <https://docs.zephyrproject.org/latest/boards/seeed/reterminal_e1002/doc/index.html>.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::spi::SpiDevice;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::spi::SpiDevice;

use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "blocking")]
use embedded_hal::digital::InputPin as Wait;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// ED2208 Command Definitions
pub mod cmd {
    /// Panel Setting command
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power Setting command
    pub const POWER_SETTING: u8 = 0x01;
    /// Power OFF command
    pub const POWER_OFF: u8 = 0x02;
    /// Power OFF Sequence Setting
    pub const POWER_OFF_SEQUENCE: u8 = 0x03;
    /// Power ON command
    pub const POWER_ON: u8 = 0x04;
    /// Booster Soft Start 1
    pub const BOOSTER_SOFT_START1: u8 = 0x05;
    /// Booster Soft Start 2
    pub const BOOSTER_SOFT_START2: u8 = 0x06;
    /// Deep Sleep command
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Booster Soft Start 3
    pub const BOOSTER_SOFT_START3: u8 = 0x08;
    /// Data Start Transmission / Write RAM (DTM)
    pub const DATA_START_TRANSMISSION: u8 = 0x10;
    /// Display Refresh command (DRF)
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// PLL Control
    pub const PLL_CONTROL: u8 = 0x30;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// TCON Setting
    pub const TCON: u8 = 0x60;
    /// Resolution Setting (TRES)
    pub const RESOLUTION: u8 = 0x61;
    /// Partial Window Setting
    pub const PARTIAL_WINDOW: u8 = 0x83;
    /// Temperature sensor VDCS
    pub const T_VDCS: u8 = 0x84;
    /// Command Header / Unlock (CMDH)
    pub const CMDH: u8 = 0xAA;
    /// Power Saving Setting (PWS)
    pub const PWS: u8 = 0xE3;
}

/// ED2208 Controller IC driver implementation.
#[derive(Debug, Clone, Copy)]
pub struct Ed2208Controller {
    width: u32,
    height: u32,
}

impl Ed2208Controller {
    /// Creates a new ED2208 controller configured for target display dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl Ed2208Controller {
    /// Sets partial RAM area window on the controller.
    #[allow(clippy::type_complexity)]
    pub async fn set_partial_ram_area<SPI, DC, RST, BUSY>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x: u32,
        y: u32,
        w: u32,
        h: u32,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        let xe = x + w - 1;
        let ye = y + h;
        bus.send_command_with_data(
            cmd::PARTIAL_WINDOW,
            &[
                (x / 256) as u8,
                (x % 256) as u8,
                (xe / 256) as u8,
                (xe % 256) as u8,
                (y / 256) as u8,
                (y % 256) as u8,
                (ye / 256) as u8,
                (ye % 256) as u8,
                0x01,
            ],
        )
        .await
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Ed2208Controller
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
        // Hardware reset pulse
        bus.hard_reset(delay, 20).await?;

        // Vendor unlock sequence (CMDH)
        bus.send_command_with_data(cmd::CMDH, &[0x49, 0x55, 0x20, 0x08, 0x09, 0x18])
            .await?;

        // Power setting (PWRR)
        bus.send_command_with_data(cmd::POWER_SETTING, &[0x3F])
            .await?;

        // Panel setting (PSR)
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x5F, 0x69])
            .await?;

        // Power offset (POFS)
        bus.send_command_with_data(cmd::POWER_OFF_SEQUENCE, &[0x00, 0x54, 0x00, 0x44])
            .await?;

        // Booster soft start stages
        bus.send_command_with_data(cmd::BOOSTER_SOFT_START1, &[0x40, 0x1F, 0x1F, 0x2C])
            .await?;
        bus.send_command_with_data(cmd::BOOSTER_SOFT_START2, &[0x6F, 0x1F, 0x17, 0x49])
            .await?;
        bus.send_command_with_data(cmd::BOOSTER_SOFT_START3, &[0x6F, 0x1F, 0x1F, 0x22])
            .await?;

        // PLL control
        bus.send_command_with_data(cmd::PLL_CONTROL, &[0x08])
            .await?;

        // VCOM & data interval setting (CDI)
        bus.send_command_with_data(cmd::CDI, &[0x3F]).await?;

        // TCON setting
        bus.send_command_with_data(cmd::TCON, &[0x02, 0x00])
            .await?;

        // Resolution setting (TRES)
        let w = self.width;
        let h = self.height;
        bus.send_command_with_data(
            cmd::RESOLUTION,
            &[
                (w >> 8) as u8,
                (w & 0xFF) as u8,
                (h >> 8) as u8,
                (h & 0xFF) as u8,
            ],
        )
        .await?;

        // Temperature VDCS setting
        bus.send_command_with_data(cmd::T_VDCS, &[0x01]).await?;

        // Power saving setting (PWS)
        bus.send_command_with_data(cmd::PWS, &[0x2F]).await?;

        // Power ON and wait while busy (busy active-low)
        bus.send_command(cmd::POWER_ON).await?;
        bus.wait_busy(false).await?;

        Ok(())
    }

    async fn set_window(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error> {
        let w = x_end.saturating_sub(x_start) + 1;
        let h = y_end.saturating_sub(y_start) + 1;
        self.set_partial_ram_area(bus, x_start, y_start, w, h)
            .await
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
        let buf = [byte; 64];
        let mut remaining = count;
        while remaining > 0 {
            let chunk_len = remaining.min(buf.len());
            bus.send_data(&buf[..chunk_len]).await?;
            remaining -= chunk_len;
        }
        Ok(())
    }

    async fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::DISPLAY_REFRESH, &[0x00])
            .await?;
        delay.delay_ms(1).await;
        // Busy active-low during full display refresh
        bus.wait_busy_with_delay(delay, false).await
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // Power off command
        bus.send_command_with_data(cmd::POWER_OFF, &[0x00]).await?;
        bus.wait_busy(false).await?;

        // Deep sleep command
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5]).await
    }
}

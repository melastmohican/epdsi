//! UC8175 E-Paper Display Controller implementation.
//!
//! The UC8175 is UltraChip's miniature monochrome controller for the 1.02" class of panels. Its
//! command core is the UC81xx family's (PSR, PON, DRF, DTM1/DTM2, CDI, DSLP), but the vendor
//! init is much shorter than the [`Uc8151dController`](super::uc8151d::Uc8151dController) one:
//! the resolution comes from two bits of the panel setting instead of a `TRES` command, and the
//! waveform always comes from the chip's OTP.
//!
//! ## Provenance
//!
//! Every byte below comes from Good Display's own `GDEW0102I4FC` package
//! (`AU-GDEW0102I4FC-20250926`, `Display_EPD_W21.cpp`), checked against `GDEW0102I4FC.pdf` for the
//! panel setting bit layout. Bench verified on 2 Nov 2026 with the blocking API on an XIAO ESP32-C3 wired by hand to a `DESPI-C102` adapter: the full refresh rendered the Rust and Ferris picture.
//!
//! ## Things to know
//!
//! - **BUSY is active-LOW** (idle high). Pass a pull-up input.
//! - **RAM planes**: `ColorChannel::BlackWhite` writes `0x13` (new data), and
//!   `ColorChannel::RedYellow` writes `0x10` (old data). White is `1`. Prime the old plane to
//!   `0xFF` before the refresh, as the vendor demo does.
//! - **Panel setting `0x5F`** selects 80 x 128 (`RES` = `01`), the OTP LUT (`REG` = 0) and the
//!   normal scan direction. [`Uc8175Controller::with_rotated_180`] sends `0x53` instead, the
//!   vendor's `EPD_Init_180`, which flips the scan direction (`UD` and `SHL` cleared).
//! - **Command `0x2A`** with two zero bytes is an undocumented "hidden instruction" in the vendor
//!   demo. It is not in the panel specification. It is sent as the demo sends it.
//! - **No partial refresh.** The vendor demo has only the full update, and the panel is rated
//!   for a 3 s full refresh.

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

/// UC8175 command definitions.
pub mod cmd {
    /// Panel Setting
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power OFF
    pub const POWER_OFF: u8 = 0x02;
    /// Power ON
    pub const POWER_ON: u8 = 0x04;
    /// Deep Sleep
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Write the "old" (previous image) data plane
    pub const WRITE_OLD_DATA: u8 = 0x10;
    /// Display Refresh
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// Write the "new" (current image) data plane
    pub const WRITE_NEW_DATA: u8 = 0x13;
    /// Undocumented "IC hidden instruction" the vendor demo sends during init
    pub const HIDDEN_2A: u8 = 0x2A;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
}

/// Panel setting for the normal orientation: 80 x 128, OTP LUT.
const PSR_NORMAL: u8 = 0x5F;
/// Panel setting for the 180 degree orientation (the vendor's `EPD_Init_180`).
const PSR_ROTATED_180: u8 = 0x53;

/// How long to wait for BUSY to assert after `DISPLAY_REFRESH`, before giving up and waiting for
/// completion anyway. Polling straight away can read "idle" before the chip has started.
const BUSY_ASSERT_TIMEOUT_MS: u32 = 500;

/// UC8175 controller IC driver configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Uc8175Controller {
    rotated_180: bool,
}

impl Uc8175Controller {
    /// Creates a new UC8175 controller in the normal orientation.
    pub fn new() -> Self {
        Self::default()
    }

    /// Selects the 180 degree orientation (builder method). Applies at the next `init`.
    pub fn with_rotated_180(mut self, rotated: bool) -> Self {
        self.rotated_180 = rotated;
        self
    }

    /// Selects the 180 degree orientation. Applies at the next `init`.
    pub fn set_rotated_180(&mut self, rotated: bool) {
        self.rotated_180 = rotated;
    }

    /// Returns `true` when the 180 degree orientation is selected.
    pub fn rotated_180(&self) -> bool {
        self.rotated_180
    }

    /// Maps a colour channel onto the RAM plane command: Black/White to the "new" plane `0x13`,
    /// the second channel to the "old" plane `0x10`.
    fn plane_command(channel: ColorChannel) -> u8 {
        if matches!(channel, ColorChannel::BlackWhite) {
            cmd::WRITE_NEW_DATA
        } else {
            cmd::WRITE_OLD_DATA
        }
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Uc8175Controller
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
        bus.hard_reset(delay, 10).await?;
        bus.wait_busy_with_delay(delay, false).await?;

        let psr = if self.rotated_180 {
            PSR_ROTATED_180
        } else {
            PSR_NORMAL
        };
        bus.send_command_with_data(cmd::PANEL_SETTING, &[psr])
            .await?;
        bus.send_command_with_data(cmd::HIDDEN_2A, &[0x00, 0x00])
            .await?;
        bus.send_command(cmd::POWER_ON).await?;
        bus.wait_busy_with_delay(delay, false).await?;
        bus.send_command_with_data(cmd::CDI, &[0x97]).await
    }

    async fn set_window(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x_start: u32,
        _y_start: u32,
        _x_end: u32,
        _y_end: u32,
    ) -> Result<(), Self::Error> {
        // Full-frame only: the vendor demo has no partial window.
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
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(Self::plane_command(channel), data)
            .await
    }

    async fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        bus.send_command(Self::plane_command(channel)).await?;
        bus.send_data_repeated(byte, count).await
    }

    async fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::DISPLAY_REFRESH).await?;
        // The vendor demo says this gap is necessary, 200 us at least.
        delay.delay_ms(1).await;
        bus.wait_busy_assert(delay, false, BUSY_ASSERT_TIMEOUT_MS)
            .await?;
        bus.wait_busy_with_delay(delay, false).await
    }

    async fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::CDI, &[0xF7]).await?;
        bus.send_command(cmd::POWER_OFF).await?;
        bus.wait_busy_with_delay(delay, false).await?;
        // The vendor demo gives this gap before deep sleep, 100 ms.
        delay.delay_ms(100).await;
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5]).await
    }
}

//! UC8253 E-Paper Display Controller implementation.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// UC8253 Command Definitions
pub mod cmd {
    /// Panel Setting command
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power OFF command
    pub const POWER_OFF: u8 = 0x02;
    /// Power ON command
    pub const POWER_ON: u8 = 0x04;
    /// Deep Sleep command
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Write "old"/previous image data plane
    pub const WRITE_OLD_DATA: u8 = 0x10;
    /// Display Refresh command
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// Write "new"/current image data plane
    pub const WRITE_NEW_DATA: u8 = 0x13;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// Cascade Setting (CCSET), used to force a temperature-compensated fast waveform speed
    pub const CASCADE_SETTING: u8 = 0xE0;
    /// Force Temperature (TSSET), used together with `CASCADE_SETTING` for fast update modes
    pub const FORCE_TEMP: u8 = 0xE5;
    /// Partial Window In command
    pub const PARTIAL_IN: u8 = 0x91;
    /// Partial Window command
    pub const PARTIAL_WINDOW: u8 = 0x90;
    /// Partial Window Out command
    pub const PARTIAL_OUT: u8 = 0x92;
}

/// Display refresh operating mode for the UC8253 controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Uc8253RefreshMode {
    /// Full display update using the panel's default waveform speed (CDI `0x97`).
    #[default]
    Full,
    /// Full display update forced to a faster temperature-compensated waveform (CCSET/TSSET `0x5A`, CDI `0x97`).
    FastFull,
    /// Partial display update (CDI `0xD7`).
    Partial,
    /// Partial display update forced to a faster temperature-compensated waveform (CCSET/TSSET `0x6E`, CDI `0xD7`).
    FastPartial,
}

/// UC8253 Controller IC driver configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Uc8253Controller {
    width: u32,
    height: u32,
    refresh_mode: Uc8253RefreshMode,
    /// Last window set via `set_window`, tracked so `trigger_refresh` knows whether to close
    /// partial-window mode (`PARTIAL_OUT`) before refreshing.
    window: Option<(u32, u32, u32, u32)>,
}

impl Uc8253Controller {
    /// Creates a new UC8253 controller configured for target dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            refresh_mode: Uc8253RefreshMode::default(),
            window: None,
        }
    }

    /// Sets display refresh operating mode (builder method).
    pub fn with_refresh_mode(mut self, mode: Uc8253RefreshMode) -> Self {
        self.refresh_mode = mode;
        self
    }

    /// Sets display refresh operating mode.
    pub fn set_refresh_mode(&mut self, mode: Uc8253RefreshMode) {
        self.refresh_mode = mode;
    }

    /// Returns current display refresh mode.
    pub fn refresh_mode(&self) -> Uc8253RefreshMode {
        self.refresh_mode
    }

    /// Clears any recorded partial window, returning the controller to full-frame addressing.
    ///
    /// Full-frame writes must not be wrapped in a partial-window session, so call this before a
    /// full-screen write/refresh that follows partial updates.
    pub fn clear_window(&mut self) {
        self.window = None;
    }

    /// Opens the recorded partial window (`PARTIAL_IN` + `PARTIAL_WINDOW`), if one is set.
    ///
    /// The UC8253 requires the window to be re-opened around *each* RAM write and again around
    /// the refresh, so this is emitted per operation rather than once by `set_window`.
    #[allow(clippy::type_complexity)]
    fn open_window<SPI, DC, RST, BUSY>(
        &self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        let Some((x_start, y_start, x_end, y_end)) = self.window else {
            return Ok(());
        };

        let x = (x_start & 0xFFF8) as u8;
        let xe = (x_end | 0x0007) as u8;
        let y = y_start as u16;
        let ye = y_end as u16;

        bus.send_command(cmd::PARTIAL_IN)?;
        bus.send_command_with_data(
            cmd::PARTIAL_WINDOW,
            &[
                x,
                xe,
                (y / 256) as u8,
                (y % 256) as u8,
                (ye / 256) as u8,
                (ye % 256) as u8,
                0x01,
            ],
        )
    }

    /// Closes the partial window (`PARTIAL_OUT`), if one is set.
    #[allow(clippy::type_complexity)]
    fn close_window<SPI, DC, RST, BUSY>(
        &self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
    {
        if self.window.is_some() {
            bus.send_command(cmd::PARTIAL_OUT)?;
        }
        Ok(())
    }

    /// Re-sends the two Panel Setting register writes performed during `init_sequence`, used to
    /// undo the CCSET/TSSET "TSFIX" temperature override after a fast-update refresh.
    #[allow(clippy::type_complexity)]
    fn reset_panel_setting<SPI, DC, RST, BUSY, DELAY>(
        &self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
        DELAY: DelayNs,
    {
        // `0x1E` clears RST_N, which is a soft reset; the controller needs time to settle before
        // `0x1F` releases it. Without the wait the reset's power-on defaults win and the scan
        // direction flips, rotating every subsequent frame by 180 degrees.
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1E, 0x0D])?;
        delay.delay_ms(1);
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1F, 0x0D])
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Uc8253Controller
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
        // `0x1E` clears RST_N (soft reset); the 1 ms wait lets it settle before `0x1F` releases it.
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1E, 0x0D])?;
        delay.delay_ms(1);
        bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1F, 0x0D])?;
        Ok(())
    }

    /// Records the partial window without emitting any commands.
    ///
    /// The UC8253 needs `PARTIAL_IN` + `PARTIAL_WINDOW` re-issued around every RAM write and
    /// again around the refresh, so the commands are emitted by `write_frame`,
    /// `write_frame_pattern` and `trigger_refresh`. Use [`Uc8253Controller::clear_window`] to
    /// return to full-frame addressing.
    fn set_window(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), Self::Error> {
        self.window = Some((x_start, y_start, x_end, y_end));
        Ok(())
    }

    fn set_cursor(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x: u32,
        _y: u32,
    ) -> Result<(), Self::Error> {
        // UC8253 has no separate RAM cursor register; window position is set by `set_window`.
        Ok(())
    }

    fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        let cmd = match channel {
            ColorChannel::BlackWhite => cmd::WRITE_NEW_DATA,
            ColorChannel::RedYellow
            | ColorChannel::Red
            | ColorChannel::Yellow
            | ColorChannel::Color7(_) => cmd::WRITE_OLD_DATA,
        };
        self.open_window(bus)?;
        bus.send_command_with_data(cmd, data)?;
        self.close_window(bus)
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        let cmd = match channel {
            ColorChannel::BlackWhite => cmd::WRITE_NEW_DATA,
            ColorChannel::RedYellow
            | ColorChannel::Red
            | ColorChannel::Yellow
            | ColorChannel::Color7(_) => cmd::WRITE_OLD_DATA,
        };
        self.open_window(bus)?;
        bus.send_command(cmd)?;
        bus.send_data_repeated(byte, count)?;
        self.close_window(bus)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // The refresh gets its own partial-window session, re-opened here and closed after the
        // update. The window is deliberately kept for subsequent operations; call
        // `clear_window` to return to full-frame addressing.
        self.open_window(bus)?;

        let is_fast = matches!(
            self.refresh_mode,
            Uc8253RefreshMode::FastFull | Uc8253RefreshMode::FastPartial
        );
        let is_partial = matches!(
            self.refresh_mode,
            Uc8253RefreshMode::Partial | Uc8253RefreshMode::FastPartial
        );

        if is_fast {
            let temp = if is_partial { 0x6E } else { 0x5A };
            bus.send_command_with_data(cmd::CASCADE_SETTING, &[0x02])?;
            bus.send_command_with_data(cmd::FORCE_TEMP, &[temp])?;
        }

        let cdi = if is_partial { 0xD7 } else { 0x97 };
        bus.send_command_with_data(cmd::CDI, &[cdi])?;

        bus.send_command(cmd::POWER_ON)?;
        bus.wait_busy_with_delay(delay, false)?;
        bus.send_command(cmd::DISPLAY_REFRESH)?;
        bus.wait_busy_with_delay(delay, false)?;
        bus.send_command(cmd::POWER_OFF)?;
        bus.wait_busy_with_delay(delay, false)?;

        if is_fast {
            self.reset_panel_setting(bus, delay)?;
        }

        self.close_window(bus)
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::POWER_OFF)?;
        bus.wait_busy_with_delay(delay, false)?;
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5])
    }
}

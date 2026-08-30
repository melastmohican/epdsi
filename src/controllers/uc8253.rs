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
    /// Booster Soft Start command
    pub const BOOSTER_SOFT_START: u8 = 0x06;
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
    /// Resolution Setting (TRES): HRES byte, then VRES high/low bytes
    pub const RESOLUTION: u8 = 0x61;
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

/// How long to wait for BUSY to assert after a command, before giving up and waiting for
/// completion anyway.
///
/// Generous relative to the ~17 s refresh it guards. A panel that has not asserted within half a
/// second is not going to, and falling through leaves the "missing panel reads idle" behaviour
/// intact rather than hanging.
const BUSY_ASSERT_TIMEOUT_MS: u32 = 500;

/// Panel register profile for the UC8253 controller.
///
/// The UC8253 drives panels whose init sequences, RAM plane assignment and refresh handling
/// differ enough that one profile cannot serve both. Selecting the wrong variant does not
/// error — it renders inverted or blank — so the variant must match the panel type the
/// [`crate::driver::EpdDriver`] is built with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Uc8253Variant {
    /// Good Display `GDEY037T03` profile (GxEPD2 lineage).
    ///
    /// Monochrome; `PANEL_SETTING` soft-reset init; Black/White plane on
    /// [`cmd::WRITE_NEW_DATA`]; every refresh powers the panel up and back down, and re-issues
    /// `CDI`. Supports all four [`Uc8253RefreshMode`] modes.
    #[default]
    Gdey037t03,
    /// Waveshare `SE0352N14-TNG-A0` (3.52" e-Paper HAT (B)) profile.
    ///
    /// Tri-Color; explicit `POWER_ON`/`CDI`/`RESOLUTION`/`BOOSTER_SOFT_START` init; Black/White
    /// plane on [`cmd::WRITE_OLD_DATA`] — swapped relative to [`Uc8253Variant::Gdey037t03`];
    /// refresh is `POWER_ON` then `DISPLAY_REFRESH`, without re-issuing `CDI`.
    ///
    /// Full refresh only (~16–20 s): the red pigment has no differential waveform, so
    /// [`Uc8253RefreshMode`] is ignored for this variant.
    ///
    /// The controller drops its charge pump after an update, so each refresh powers it back on.
    /// Waveshare's reference driver does this by re-running its entire init before every display
    /// operation — one refresh per init. Skipping it does not error: the `DISPLAY_REFRESH` is
    /// silently ignored, BUSY never asserts, and the refresh appears to complete instantly having
    /// drawn nothing.
    Se0352n14,
}

/// Display refresh operating mode for the UC8253 controller.
///
/// Ignored by [`Uc8253Variant::Se0352n14`], which is full-refresh only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    variant: Uc8253Variant,
    refresh_mode: Uc8253RefreshMode,
    /// Last window set via `set_window`, tracked so `trigger_refresh` knows whether to close
    /// partial-window mode (`PARTIAL_OUT`) before refreshing.
    window: Option<(u32, u32, u32, u32)>,
}

impl Uc8253Controller {
    /// Creates a new UC8253 controller configured for target dimensions.
    ///
    /// Defaults to [`Uc8253Variant::Gdey037t03`]; use [`Self::with_variant`] for other panels.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            variant: Uc8253Variant::default(),
            refresh_mode: Uc8253RefreshMode::default(),
            window: None,
        }
    }

    /// Sets the panel register profile (builder method).
    pub fn with_variant(mut self, variant: Uc8253Variant) -> Self {
        self.variant = variant;
        self
    }

    /// Returns the configured panel register profile.
    pub fn variant(&self) -> Uc8253Variant {
        self.variant
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

    /// Maps a colour channel onto the RAM plane command for the configured variant.
    ///
    /// The two planes are swapped between variants: the `GDEY037T03` profile treats
    /// `WRITE_NEW_DATA` as the Black/White plane and `WRITE_OLD_DATA` as the previous-frame plane
    /// used for differential updates, while the tri-colour `SE0352N14-TNG-A0` puts Black/White on
    /// `WRITE_OLD_DATA` and the red pigment on `WRITE_NEW_DATA`. Crossing them routes black pixels
    /// into the red plane.
    fn plane_command(&self, channel: ColorChannel) -> u8 {
        let is_black_white = matches!(channel, ColorChannel::BlackWhite);
        match self.variant {
            Uc8253Variant::Gdey037t03 => {
                if is_black_white {
                    cmd::WRITE_NEW_DATA
                } else {
                    cmd::WRITE_OLD_DATA
                }
            }
            Uc8253Variant::Se0352n14 => {
                if is_black_white {
                    cmd::WRITE_OLD_DATA
                } else {
                    cmd::WRITE_NEW_DATA
                }
            }
        }
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
        match self.variant {
            Uc8253Variant::Gdey037t03 => {
                bus.hard_reset(delay, 10)?;
                // `0x1E` clears RST_N (soft reset); the 1 ms wait lets it settle before `0x1F`
                // releases it.
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1E, 0x0D])?;
                delay.delay_ms(1);
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x1F, 0x0D])?;
            }
            Uc8253Variant::Se0352n14 => {
                // A 30 ms RST low pulse, matching Pervasive Displays' reference driver for this
                // panel family. Waveshare's uses 2 ms, which proved marginal: on a XIAO ESP32-C3
                // it latched intermittently, and a reset that does not take leaves the controller
                // ignoring POWER_ON and DISPLAY_REFRESH — BUSY never asserts and the refresh
                // returns having drawn nothing, at a random frame each run.
                //
                // `hard_reset` does not cover the settle that must follow, so that is added here.
                bus.hard_reset(delay, 30)?;
                delay.delay_ms(200);

                // Power comes up here rather than per-refresh: this panel's refresh is a bare
                // `DISPLAY_REFRESH`, so the rails stay up between frames.
                bus.send_command(cmd::POWER_ON)?;
                delay.delay_ms(100);
                bus.wait_busy_with_delay(delay, false)?;

                // `0x87` — not the `0x97` the GDEY037T03 profile uses. The two differ in the DDX
                // polarity bits, and `0x87` is what makes `0x00` mean white in both RAM planes on
                // this panel. Re-issuing `0x97` later would invert black and white.
                bus.send_command_with_data(cmd::CDI, &[0x87])?;
                bus.send_command_with_data(cmd::PANEL_SETTING, &[0x03, 0x0D])?;
                // Unlike the GDEY037T03, which encodes its size in the Panel Setting bits, this
                // panel needs an explicit resolution: HRES ignores the low 3 bits, VRES is 16-bit.
                bus.send_command_with_data(
                    cmd::RESOLUTION,
                    &[
                        (self.width & 0xF8) as u8,
                        (self.height >> 8) as u8,
                        (self.height & 0xFF) as u8,
                    ],
                )?;
                bus.send_command_with_data(cmd::BOOSTER_SOFT_START, &[0x2F, 0x2F, 0x2E])?;
                bus.wait_busy_with_delay(delay, false)?;
            }
        }
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
        let cmd = self.plane_command(channel);
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
        let cmd = self.plane_command(channel);
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

        if self.variant == Uc8253Variant::Se0352n14 {
            // `CDI` is deliberately not re-issued — `0x97` would move the DDX polarity bits away
            // from the `0x87` set at init and invert black and white. `refresh_mode` has no
            // effect here either: tri-colour ink has no differential waveform.
            //
            // Power on before every refresh. Waveshare's reference driver re-runs its whole
            // init — which begins with POWER_ON — before each display operation, exactly one
            // refresh per init. The controller drops its charge pump after an update, and a bare
            // DISPLAY_REFRESH on an unpowered controller is silently ignored: BUSY never asserts,
            // the poll reads idle, and the refresh "finishes" instantly having done nothing.
            //
            // The fixed 100 ms settle matches both the reference driver and this controller's own
            // `init_sequence`. Watching for a BUSY edge here instead is worse: `POWER_ON` often
            // does not assert BUSY at all on this panel, so the edge wait just burns its timeout,
            // and when BUSY *does* blip low during a ramp it can release the wait before the
            // booster has stabilised.
            bus.send_command(cmd::POWER_ON)?;
            delay.delay_ms(100);
            bus.wait_busy_with_delay(delay, false)?;

            // Wait for BUSY to actually assert before waiting for it to clear. A fixed settling
            // delay is not sufficient: assertion latency varies, and a 10 ms guard was observed
            // holding on some refreshes and missing on others on the same board, leaving the
            // panel a frame behind whenever it missed.
            bus.send_command(cmd::DISPLAY_REFRESH)?;
            bus.wait_busy_assert(delay, false, BUSY_ASSERT_TIMEOUT_MS)?;
            bus.wait_busy_with_delay(delay, false)?;
            return self.close_window(bus);
        }

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

        // Wait for the BUSY edge before waiting for completion. On a host that polls quickly this
        // is what separates "the refresh finished" from "the panel had not started yet" — without
        // it a refresh can report 0 ms while still running, or while the controller ignored the
        // command entirely. Costs one poll when BUSY asserts normally, so the fast-partial path is
        // unaffected; only the already-broken case pays the timeout.
        bus.send_command(cmd::DISPLAY_REFRESH)?;
        bus.wait_busy_assert(delay, false, BUSY_ASSERT_TIMEOUT_MS)?;
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

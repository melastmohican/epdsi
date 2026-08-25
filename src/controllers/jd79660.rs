//! JD79660 E-Paper Display Controller implementation.
//!
//! Register sequence matches GxEPD2 `GxEPD2_154c_GDEM0154F51H`:
//! `_InitDisplay`, `_refresh`, `_setPartialRamArea`, `_PowerOff`, `hibernate`.
//! Do not reuse [`Jd79661Controller`](crate::controllers::Jd79661Controller): the
//! OTP values differ (PSR, booster, fast-LUT, refresh framing).
//!
//! After every refresh GxEPD2 sets `_init_display_done = false` and re-runs
//! `_InitDisplay()` on the next RAM write (reset is skipped unless hibernating).
//! This controller does the same for the post-refresh path. After [`EpdController::sleep`]
//! the chip is in deep sleep (`_hibernating`); GxEPD2 then HW-resets inside the next
//! `_InitDisplay`. Call [`EpdController::init_sequence`] again — `write_frame` has no
//! `DelayNs` and cannot pulse RST.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// JD79660 command definitions.
pub mod cmd {
    /// Panel Setting (PSR)
    pub const PANEL_SETTING: u8 = 0x00;
    /// Power OFF
    pub const POWER_OFF: u8 = 0x02;
    /// Power ON
    pub const POWER_ON: u8 = 0x04;
    /// Booster Soft Start (BTST_P)
    pub const BOOSTER_SOFT_START: u8 = 0x06;
    /// Deep Sleep
    pub const DEEP_SLEEP: u8 = 0x07;
    /// Data Start Transmission / Write RAM (2 bpp packed)
    pub const DATA_START_TRANSMISSION: u8 = 0x10;
    /// Display Refresh
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// PLL / VCOM control
    pub const VCOM_CONTROL: u8 = 0x30;
    /// Vendor unlock key
    pub const MAGIC_KEY: u8 = 0x4D;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// Resolution Setting (TRES)
    pub const RESOLUTION: u8 = 0x61;
    /// Partial RAM window (`GxEPD2` `_setPartialRamArea`)
    pub const PARTIAL_WINDOW: u8 = 0x83;
    /// Force-temperature / busy handshake after fast-LUT load
    pub const FORCE_TEMPERATURE: u8 = 0xA5;
    /// Cascade setting (fast full-update enable)
    pub const CASCADE_SETTING: u8 = 0xE0;
    /// Temperature sensor setting (fast full-update)
    pub const TSSET: u8 = 0xE6;
    /// IC analog block enable
    pub const IC_ANALOG: u8 = 0xE9;

    /// CDI payload for a full-window refresh (GxEPD2 `_refresh(false)`).
    pub const CDI_FULL: u8 = 0x37;
    /// CDI payload for a partial-window refresh (GxEPD2 `_refresh(true)`).
    pub const CDI_PARTIAL: u8 = 0x97;
}

/// Busy is active-low on JD79660 (`GxEPD2_EPD` ctor passes `LOW`).
const BUSY_ACTIVE_HIGH: bool = false;

type BusError<SPI, DC, RST, BUSY> = EpdBusError<
    <SPI as embedded_hal::spi::ErrorType>::Error,
    <DC as embedded_hal::digital::ErrorType>::Error,
    <RST as embedded_hal::digital::ErrorType>::Error,
    <BUSY as embedded_hal::digital::ErrorType>::Error,
>;

/// JD79660 controller IC driver (Good Display `GDEM0154F51H` / Waveshare 1.54G).
#[derive(Debug, Clone, Copy)]
pub struct Jd79660Controller {
    width: u32,
    height: u32,
    fast_full_update: bool,
    /// Last `set_window` was smaller than the panel — next refresh uses CDI `0x97`.
    partial_window: bool,
    /// Inclusive window last programmed with `0x83` (`None` = full panel, GxEPD2 `refresh(bool)`).
    window: Option<(u32, u32, u32, u32)>,
    /// GxEPD2 `_init_display_done == false` — next RAM write re-sends `_InitDisplay` registers.
    needs_reinit: bool,
    /// GxEPD2 `_power_is_on` — skip `0x04` when still powered after a refresh.
    power_on: bool,
    /// GxEPD2 `_hibernating` — deep sleep; next `_InitDisplay` must HW-reset (`init_sequence`).
    hibernating: bool,
}

impl Jd79660Controller {
    /// Creates a new JD79660 controller for the given panel dimensions.
    ///
    /// Fast full-update LUT load (`0xE0` / `0xE6` / `0xA5`) is enabled by default,
    /// matching GxEPD2 `useFastFullUpdate = true`.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            fast_full_update: true,
            partial_window: false,
            window: None,
            needs_reinit: false,
            power_on: false,
            hibernating: false,
        }
    }

    /// Enables or disables the GxEPD2 fast full-update LUT (`0xE0`/`0xE6`/`0xA5`).
    ///
    /// Set `false` for the extended (low) temperature range, as in GxEPD2
    /// `useFastFullUpdate`.
    pub fn with_fast_full_update(mut self, enabled: bool) -> Self {
        self.fast_full_update = enabled;
        self
    }

    fn is_full_window(&self, x_start: u32, y_start: u32, x_end: u32, y_end: u32) -> bool {
        x_start == 0
            && y_start == 0
            && x_end >= self.width.saturating_sub(1)
            && y_end >= self.height.saturating_sub(1)
    }
}

fn be16(v: u32) -> [u8; 2] {
    [((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8]
}

fn send_partial_window<SPI, DC, RST, BUSY>(
    bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    x_start: u32,
    y_start: u32,
    x_end: u32,
    y_end: u32,
    partial: bool,
) -> Result<(), BusError<SPI, DC, RST, BUSY>>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    let [xs_h, xs_l] = be16(x_start);
    let [xe_h, xe_l] = be16(x_end);
    let [ys_h, ys_l] = be16(y_start);
    let [ye_h, ye_l] = be16(y_end);
    bus.send_command_with_data(
        cmd::PARTIAL_WINDOW,
        &[
            xs_h,
            xs_l,
            xe_h,
            xe_l,
            ys_h,
            ys_l,
            ye_h,
            ye_l,
            if partial { 0x01 } else { 0x00 },
        ],
    )
}

/// GxEPD2 `_InitDisplay` body after the optional hardware reset.
fn write_init_registers<SPI, DC, RST, BUSY, F>(
    ctrl: &Jd79660Controller,
    bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    mut wait_idle: F,
) -> Result<(), BusError<SPI, DC, RST, BUSY>>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
    F: FnMut(&mut SpiBusWrapper<SPI, DC, RST, BUSY>) -> Result<(), BusError<SPI, DC, RST, BUSY>>,
{
    bus.send_command_with_data(cmd::MAGIC_KEY, &[0x78])?;
    bus.send_command_with_data(cmd::PANEL_SETTING, &[0x0F, 0x29])?;
    bus.send_command_with_data(
        cmd::BOOSTER_SOFT_START,
        &[0x0D, 0x12, 0x30, 0x20, 0x19, 0x2A, 0x22],
    )?;
    bus.send_command_with_data(cmd::CDI, &[cmd::CDI_FULL])?;

    let [w_high, w_low] = be16(ctrl.width);
    let [h_high, h_low] = be16(ctrl.height);
    bus.send_command_with_data(cmd::RESOLUTION, &[w_high, w_low, h_high, h_low])?;

    bus.send_command_with_data(cmd::IC_ANALOG, &[0x01])?;
    bus.send_command_with_data(cmd::VCOM_CONTROL, &[0x08])?;

    if ctrl.fast_full_update {
        bus.send_command_with_data(cmd::CASCADE_SETTING, &[0x02])?;
        bus.send_command_with_data(cmd::TSSET, &[0x5D])?;
        bus.send_command_with_data(cmd::FORCE_TEMPERATURE, &[0x00])?;
        wait_idle(bus)?;
    }
    Ok(())
}

fn power_on<SPI, DC, RST, BUSY, F>(
    ctrl: &mut Jd79660Controller,
    bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
    mut wait_idle: F,
) -> Result<(), BusError<SPI, DC, RST, BUSY>>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
    F: FnMut(&mut SpiBusWrapper<SPI, DC, RST, BUSY>) -> Result<(), BusError<SPI, DC, RST, BUSY>>,
{
    if !ctrl.power_on {
        bus.send_command(cmd::POWER_ON)?;
        wait_idle(bus)?;
        ctrl.power_on = true;
    }
    Ok(())
}

/// Lazy `_InitDisplay` after a refresh (GxEPD2: no reset, PowerOn skipped if up).
fn reinit_after_refresh<SPI, DC, RST, BUSY>(
    ctrl: &mut Jd79660Controller,
    bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
) -> Result<(), BusError<SPI, DC, RST, BUSY>>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    // Deep sleep (`0x07 0xA5`) needs RST to wake — GxEPD2 does that inside
    // `_InitDisplay` via `delay()`. `write_frame` has no `DelayNs`; call `init_sequence`.
    if ctrl.hibernating || !ctrl.needs_reinit {
        return Ok(());
    }
    write_init_registers(ctrl, bus, |b| b.wait_busy(BUSY_ACTIVE_HIGH))?;
    power_on(ctrl, bus, |b| b.wait_busy(BUSY_ACTIVE_HIGH))?;
    ctrl.needs_reinit = false;
    Ok(())
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for Jd79660Controller
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    type Error = BusError<SPI, DC, RST, BUSY>;

    fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // GxEPD2 `_InitDisplay`: HIGH ≥20 ms, LOW 2 ms, HIGH 2 ms (Waveshare clever reset).
        // `hard_reset(2)` is HIGH 5 ms + LOW/HIGH 2 ms; the extra 20 ms covers the prelude.
        delay.delay_ms(20);
        bus.hard_reset(delay, 2)?;
        delay.delay_ms(1);
        bus.wait_busy_with_delay(delay, BUSY_ACTIVE_HIGH)?;
        self.power_on = false;

        write_init_registers(self, bus, |b| {
            b.wait_busy_with_delay(delay, BUSY_ACTIVE_HIGH)
        })?;
        power_on(self, bus, |b| {
            b.wait_busy_with_delay(delay, BUSY_ACTIVE_HIGH)
        })?;
        self.needs_reinit = false;
        self.hibernating = false;
        self.partial_window = false;
        self.window = None;
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
        let full = self.is_full_window(x_start, y_start, x_end, y_end);
        self.partial_window = !full;
        self.window = Some((x_start, y_start, x_end, y_end));
        send_partial_window(bus, x_start, y_start, x_end, y_end, !full)
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
        _channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        reinit_after_refresh(self, bus)?;
        bus.send_command_with_data(cmd::DATA_START_TRANSMISSION, data)
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        reinit_after_refresh(self, bus)?;
        bus.send_command(cmd::DATA_START_TRANSMISSION)?;
        bus.send_data_repeated(byte, count)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // GxEPD2 `refresh(bool)` always programs the RAM window before `_refresh`.
        match self.window {
            Some((x0, y0, x1, y1)) => {
                send_partial_window(bus, x0, y0, x1, y1, self.partial_window)?;
            }
            None => {
                let x1 = self.width.saturating_sub(1);
                let y1 = self.height.saturating_sub(1);
                // Default `partial_mode = true` on `_setPartialRamArea(0, 0, W, H)`.
                send_partial_window(bus, 0, 0, x1, y1, true)?;
            }
        }

        let cdi = if self.partial_window {
            cmd::CDI_PARTIAL
        } else {
            cmd::CDI_FULL
        };
        bus.send_command_with_data(cmd::CDI, &[cdi])?;
        bus.send_command_with_data(cmd::DISPLAY_REFRESH, &[0x00])?;
        delay.delay_ms(1);
        bus.wait_busy_with_delay(delay, BUSY_ACTIVE_HIGH)?;
        // GxEPD2: `_init_display_done = false; // needed`
        self.needs_reinit = true;
        Ok(())
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        if self.power_on {
            bus.send_command_with_data(cmd::POWER_OFF, &[0x00])?;
            bus.wait_busy_with_delay(delay, BUSY_ACTIVE_HIGH)?;
            self.power_on = false;
        }
        bus.send_command_with_data(cmd::DEEP_SLEEP, &[0xA5])?;
        self.needs_reinit = true;
        self.hibernating = true;
        Ok(())
    }
}

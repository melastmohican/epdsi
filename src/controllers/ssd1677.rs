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
use crate::traits::{ColorChannel, EpdController, EpdPanel};

/// SSD1677 Command Definitions
pub mod cmd {
    /// Driver output control
    pub const DRIVER_CONTROL: u8 = 0x01;
    /// Gate driving voltage control
    pub const GATE_VOLTAGE: u8 = 0x03;
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
    /// Write VCOM register
    pub const WRITE_VCOM_REGISTER: u8 = 0x2C;
    /// Write LUT register (custom waveform upload)
    pub const WRITE_LUT_REGISTER: u8 = 0x32;
    /// Border waveform control
    pub const BORDER_WAVEFORM_CONTROL: u8 = 0x3C;
    /// Auto write RED RAM for a regular pattern
    pub const AUTO_WRITE_RED_RAM: u8 = 0x46;
    /// Auto write Black/White RAM for a regular pattern
    pub const AUTO_WRITE_BW_RAM: u8 = 0x47;
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
    vcom: Option<u8>,
    gate_voltage: Option<u8>,
    custom_lut: Option<&'static [u8]>,
    ram_auto_fill: bool,
    /// Visible-space origin of the last window set, so the RAM address counter can be restored
    /// after a hardware fill sweep leaves it wherever it finished.
    window_origin: (u32, u32),
}

impl Ssd1677Controller {
    /// Creates a new SSD1677 controller configured for target dimensions.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            refresh_mode: Ssd1677RefreshMode::default(),
            vcom: None,
            gate_voltage: None,
            custom_lut: None,
            ram_auto_fill: true,
            window_origin: (0, 0),
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

    /// Builds a controller from a panel type, reading its dimensions and register
    /// configuration off [`EpdPanel`].
    ///
    /// A panel declaring no [`VCOM`](EpdPanel::VCOM),
    /// [`GATE_VOLTAGE`](EpdPanel::GATE_VOLTAGE) or [`CUSTOM_LUT`](EpdPanel::CUSTOM_LUT) —
    /// which includes [`GDEQ0426T82`](crate::panels::GDEQ0426T82), the only panel this
    /// controller drives — produces a byte-identical init to [`new`](Self::new).
    ///
    /// ```
    /// use epdsi::prelude::*;
    ///
    /// let controller = Ssd1677Controller::for_panel::<GDEQ0426T82>();
    /// ```
    pub fn for_panel<P: EpdPanel>() -> Self {
        Self::new(P::WIDTH, P::HEIGHT)
            .with_vcom(P::VCOM)
            .with_gate_voltage(P::GATE_VOLTAGE)
            .with_lut(P::CUSTOM_LUT)
    }

    /// Sets the VCOM register (`0x2C`) written during init (builder method).
    ///
    /// `None` — the default — omits the write, leaving the panel on its OTP VCOM.
    ///
    /// **Do not wire a VCOM to `GDEQ0426T82`.** `GxEPD2_426_GDEQ0426T82.cpp` writes no `0x2C`
    /// anywhere; the panel runs on OTP. Ruled 1 Sep 2026 — supplying one here would be a new
    /// divergence, not a fix. The hook exists for a future SSD1677 panel whose vendor
    /// reference does write it.
    pub fn with_vcom(mut self, vcom: Option<u8>) -> Self {
        self.vcom = vcom;
        self
    }

    /// Returns the configured VCOM override, if any.
    pub fn vcom(&self) -> Option<u8> {
        self.vcom
    }

    /// Sets the gate driving voltage register (`0x03`) written during init (builder method).
    ///
    /// `None` — the default — omits the write, leaving the panel on its OTP gate voltage.
    pub fn with_gate_voltage(mut self, gate_voltage: Option<u8>) -> Self {
        self.gate_voltage = gate_voltage;
        self
    }

    /// Returns the configured gate voltage override, if any.
    pub fn gate_voltage(&self) -> Option<u8> {
        self.gate_voltage
    }

    /// Sets the custom waveform LUT uploaded to `0x32` at the end of init (builder method).
    ///
    /// `None` — the default — omits the upload, leaving the panel on its OTP waveform.
    pub fn with_lut(mut self, lut: Option<&'static [u8]>) -> Self {
        self.custom_lut = lut;
        self
    }

    /// Returns the configured custom LUT, if any.
    pub fn custom_lut(&self) -> Option<&'static [u8]> {
        self.custom_lut
    }

    /// Enables or disables the RAM auto-fill fast path for uniform frame fills (builder method).
    ///
    /// Enabled by default. Where it applies, a full-plane clear costs one command and one data
    /// byte instead of streaming every RAM byte — two bytes in place of 48,000 on an 800 × 480
    /// [`GDEQ0426T82`](crate::panels::GDEQ0426T82), per plane.
    ///
    /// # When the fast path is taken
    ///
    /// `0x46` / `0x47` drive a *regular pattern* generator, not a memset, so `epdsi` uses it only
    /// where the pattern is provably uniform. All four conditions must hold, and the byte stream
    /// is used whenever one does not:
    ///
    /// 1. the fill byte is `0x00` or `0xFF` — `A[7]` sets one step's value, so nothing else is
    ///    expressible;
    /// 2. the byte count covers a whole colour plane — the generator paints the RAM area
    ///    regardless of any count, so a partial fill would overwrite more than was asked;
    /// 3. the panel is no wider than 960 sources and no taller than 680 gates, the largest steps
    ///    `A[2:0]` and `A[6:4]` encode — beyond that the pattern alternates part-way across;
    /// 4. this flag is set.
    ///
    /// # Why it is worth knowing about
    ///
    /// **No vendor reference driver uses these registers on this panel.** Neither
    /// `GxEPD2_426_GDEQ0426T82` nor Good Display's own `GDEY0426T82` sample streams anything but
    /// bytes, and `GxEPD2_370_TC1` carries both commands commented out and marked
    /// "DON'T USE WITH GxEPD2" — a note about GxEPD2's own buffer bookkeeping, which a full-RAM
    /// sweep desynchronises, rather than a defect in the controller. `epdsi` tracks no such
    /// shadow buffer, so the objection does not carry over; the semantics implemented here come
    /// from the SSD1677 datasheet (Rev 1.0, Nov 2018) directly.
    ///
    /// That leaves this the one part of the driver with no reference implementation behind it.
    /// If a cleared panel ever comes up banded or half-inverted, switching this off restores the
    /// byte-for-byte 0.1.6 behaviour and is the first thing to try.
    ///
    /// # Ordering
    ///
    /// The sweep runs in hardware and holds BUSY high while it does; `write_frame_pattern` waits
    /// for it to clear before returning. It also leaves the RAM address counter where the sweep
    /// finished, so set the cursor before writing image data afterwards — as `render_paged`
    /// already does for every page.
    pub fn with_ram_auto_fill(mut self, enabled: bool) -> Self {
        self.ram_auto_fill = enabled;
        self
    }

    /// Returns whether the RAM auto-fill fast path is enabled.
    pub fn ram_auto_fill(&self) -> bool {
        self.ram_auto_fill
    }

    /// Number of RAM bytes one full colour plane occupies.
    fn plane_bytes(&self) -> usize {
        self.width.div_ceil(8) as usize * self.height as usize
    }

    /// Encodes the `0x46` / `0x47` parameter byte for a uniform fill, or `None` when the
    /// hardware pattern generator cannot express this fill and the caller must stream instead.
    ///
    /// The register paints a *regular alternating pattern*, not an arbitrary fill: `A[7]` is the
    /// first step's value, `A[6:4]` the step height in gates and `A[2:0]` the step width in
    /// sources. It reproduces a uniform frame only when both step sizes span the whole panel, so
    /// the value never alternates inside it. That bounds this path to panels no larger than the
    /// maximum steps — 960 sources by 680 gates — and to fills of all-ones or all-zeroes.
    fn auto_fill_pattern(&self, byte: u8, count: usize) -> Option<u8> {
        if !self.ram_auto_fill {
            return None;
        }

        // A[7]: the first step value. Only a solid 0x00 or 0xFF frame is a "pattern" with no
        // alternation; anything else has to be streamed.
        let first_step = match byte {
            0x00 => 0x00,
            0xFF => 0x80,
            _ => return None,
        };

        // The generator fills the whole RAM area, ignoring any byte count. Partial fills, and
        // callers that narrowed the window first, must keep the streaming path.
        if count != self.plane_bytes() {
            return None;
        }

        // Largest steps the register encodes: A[6:4] = 111 is 680 gates, A[2:0] = 111 is 960
        // sources. A panel exceeding either would alternate part-way across and clear to a
        // half-inverted frame rather than failing.
        const MAX_STEP_HEIGHT: u32 = 680;
        const MAX_STEP_WIDTH: u32 = 960;
        if self.height > MAX_STEP_HEIGHT || self.width > MAX_STEP_WIDTH {
            return None;
        }

        // 0xF7 for a white frame, 0x77 for a black one — 0xF7 being the byte Good Display's
        // SSD1677-family sample code uses.
        Some(first_step | (0b111 << 4) | 0b111)
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

        // Panel-declared analog overrides, in the order GxEPD2's SSD168x-family drivers write
        // them. Both absent by default — `GxEPD2_426_GDEQ0426T82` writes neither, so the only
        // panel this controller drives today emits nothing here.
        if let Some(vcom) = self.vcom {
            bus.send_command_with_data(cmd::WRITE_VCOM_REGISTER, &[vcom])?;
        }
        if let Some(gate_voltage) = self.gate_voltage {
            bus.send_command_with_data(cmd::GATE_VOLTAGE, &[gate_voltage])?;
        }

        // Set RAM Area to full display frame. `set_window` also asserts the Increment-X /
        // Decrement-Y data entry mode the reversed gates require.
        self.set_window(bus, 0, 0, self.width - 1, self.height - 1)?;
        self.set_cursor(bus, 0, 0)?;

        // Custom waveform upload goes last, matching `GxEPD2_213_B72::_Init_Full()`.
        if let Some(lut) = self.custom_lut {
            bus.send_command_with_data(cmd::WRITE_LUT_REGISTER, lut)?;
        }

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

        self.window_origin = (x_start, y_start);

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
        let is_bw = matches!(channel, ColorChannel::BlackWhite);

        // Fast path: let the controller paint the plane itself.
        if let Some(pattern) = self.auto_fill_pattern(byte, count) {
            let auto_cmd = if is_bw {
                cmd::AUTO_WRITE_BW_RAM
            } else {
                cmd::AUTO_WRITE_RED_RAM
            };
            bus.send_command_with_data(auto_cmd, &[pattern])?;
            // The datasheet is explicit that BUSY is driven high for the duration; returning
            // before it clears would let the next command land mid-sweep.
            bus.wait_busy(true)?;

            // Streaming a plane leaves the address counter wrapped back to the window origin, so
            // callers have always been able to write image data straight afterwards without
            // re-seating the cursor. A hardware sweep gives no such guarantee about where the
            // counter stops, so put it back — otherwise swapping in this path would render the
            // next frame displaced rather than faster.
            let (x, y) = self.window_origin;
            return self.set_cursor(bus, x, y);
        }

        let cmd = if is_bw {
            cmd::WRITE_BW_DATA
        } else {
            cmd::WRITE_RED_DATA
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

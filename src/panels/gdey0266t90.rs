//! Panel specification for GDEY0266T90 (2.66" 152x296 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor Hardware**: Good Display `GDEY0266T90`, sold by Waveshare as the *2.66inch e-Paper*
//!   module (SKU 18401). This is a **different, monochrome-only** glass from the Tri-Color
//!   [`GDEY0266Z90`](crate::panels::GDEY0266Z90) — same nominal size, not a config of it.
//! - **Controller IC**: SSD1680, driven through the default
//!   [`Ssd168xVariant::Ssd1680`](crate::controllers::Ssd168xVariant::Ssd1680) profile — the same
//!   one [`GDEM0213B74`](crate::panels::GDEM0213B74) and `GDEY0266Z90` use. No variant selection or
//!   panel-declared register override (`VCOM`/`CUSTOM_LUT`/`GATE_VOLTAGE`) is needed: the GxEPD2
//!   reference driver's `_InitDisplay()` writes only the border waveform (`0x3C = 0x05`), display
//!   update control (`0x21 = [0x00, 0x80]`) and temperature sensor (`0x18 = 0x80`) bytes `epdsi`
//!   already sends unconditionally for every SSD1680 panel.
//! - **Native Resolution**: 152 x 296 pixels. Waveshare advertise the panel as 296 x 152 — that is
//!   the landscape viewing orientation, not the raster, the same convention `GDEY0266Z90` uses.
//! - **RAM Alignment**: none needed. 152 is a multiple of 8, so a row is exactly 19 bytes and a
//!   frame 5624 bytes.
//! - **Busy Polarity**: Active-**HIGH** (busy while HIGH), matching every other SSD1680 panel here.
//! - **Refresh**: unlike the Tri-Color `GDEY0266Z90`, this panel is genuinely fast. The GxEPD2
//!   reference driver declares `hasPartialUpdate = true` **and** `hasFastPartialUpdate = true`, with
//!   a quoted `full_refresh_time` of 1700 ms and `partial_refresh_time` of 500 ms — so
//!   [`Ssd168xRefreshMode::Partial`](crate::controllers::Ssd168xRefreshMode::Partial) is a real
//!   differential mode here, not the parity-only no-op it is on the colour sibling.
//!   [`Ssd168xRefreshMode::FastFull`](crate::controllers::Ssd168xRefreshMode::FastFull) is also
//!   supported (`useFastFullUpdate = true` in the reference driver). Not yet measured on this
//!   crate's own hardware — see the crate's `implement-changes` plan for the pending hardware gate.
//! - **Status**: register sequence derived from the GxEPD2 reference driver
//!   (`GxEPD2_266_GDEY0266T90.cpp`) and cross-checked against `epdsi`'s existing SSD1680 controller
//!   byte-for-byte; **not yet verified on physical hardware**.
//!
//! ### Vendor References
//! - Good Display product page: <https://www.good-display.com/product/412.html>
//! - Waveshare product page: <https://www.waveshare.com/2.66inch-e-Paper.htm>
//! - Waveshare wiki: <http://www.waveshare.com/wiki/2.66inch_e-Paper_Module>
//! - Waveshare manual: <http://www.waveshare.com/wiki/2.66inch_e-Paper_Module_Manual>
//! - Datasheet: <https://files.waveshare.com/upload/d/dc/2.66inch-e-paper-specification.pdf>
//! - Waveshare reference driver: <https://github.com/waveshareteam/e-Paper/blob/master/RaspberryPi_JetsonNano/c/lib/e-Paper/EPD_2in66.c>
//! - GxEPD2 reference driver: <https://github.com/ZinggJM/GxEPD2/blob/master/src/gdey/GxEPD2_266_GDEY0266T90.h>

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for the Good Display GDEY0266T90 / Waveshare 2.66" e-Paper
/// Module (SSD1680).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEY0266T90;

/// GxEPD2 reference alias for this panel (`GxEPD2_266_GDEY0266T90`).
#[allow(non_camel_case_types)]
pub type GxEPD2_266_GDEY0266T90 = GDEY0266T90;

impl EpdPanel for GDEY0266T90 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 152;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 296;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}

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
//!   a quoted `full_refresh_time` of 1700 ms and `partial_refresh_time` of 500 ms.
//!   [`Ssd168xRefreshMode::Partial`](crate::controllers::Ssd168xRefreshMode::Partial) is confirmed
//!   on hardware — blocking on RP2350, RP2040 and ESP32-C3, async on RP2350 — to be a real
//!   differential mode here, not the parity-only no-op it is on the colour sibling — **but
//!   measured at ~4.1 s per update on the original RP2350 unit, not sub-second**, so the GxEPD2
//!   timing figure does not carry over; treat it as panel/glass-dependent rather than assumed.
//!   [`Ssd168xRefreshMode::FastFull`](crate::controllers::Ssd168xRefreshMode::FastFull) is also
//!   supported (`useFastFullUpdate = true` in the reference driver), but has not been cleanly
//!   measured on this crate's own hardware yet — the one run that exercised it was confounded by an
//!   unrelated example-side buffer bug (fixed; see `rust-rpico2-discovery`'s
//!   `ssd1680_gdey0266t90_epd` history), not a fault in `FastFull` itself.
//! - **Status**: register sequence derived from the GxEPD2 reference driver
//!   (`GxEPD2_266_GDEY0266T90.cpp`) and cross-checked against `epdsi`'s existing SSD1680 controller
//!   byte-for-byte. **Confirmed on physical hardware**, blocking on RP2350, RP2040 and ESP32-C3 and
//!   async on RP2350: `Full` and `Partial` both render correctly on every host.
//! - **4-level grayscale (Gray4)**: `GRAY4` on this panel is **not** Good Display/Waveshare
//!   material — Waveshare's own spec lists 2 grayscale levels, and GxEPD2's reference driver never
//!   writes a grayscale LUT. It is transcribed verbatim from Adafruit_EPD's
//!   `ThinkInk_266_Grayscale4_MFGN` reference driver (`ti_266mfgn_gray4_init_code` /
//!   `ti_266mfgn_gray4_lut_code`), originally confirmed rendering four distinct gray levels on a
//!   XIAO MG24 running that Arduino sketch, and now **also confirmed through `epdsi`'s own
//!   from-scratch, init-once port** — blocking on RP2350, RP2040 and ESP32-C3, async on RP2350
//!   (`ssd1680_gdey0266t90_gray4_epd`/`epdsi_ssd1680_gdey0266t90_gray4` across this crate's example
//!   repos) — all four levels render distinctly on real glass, on every host tested.
//!   Use it via `Ssd1680Controller::for_panel::<GDEY0266T90>().with_gray4(GDEY0266T90::GRAY4)`
//!   together with [`Ssd168xRefreshMode::Gray4`](crate::controllers::Ssd168xRefreshMode::Gray4).
//!   One register in the bundle (`0x3F`, "Option for LUT end") is undocumented by Adafruit but is a
//!   real SSD1680 datasheet register — see [`Gray4Registers`].
//!
//! ### Vendor References
//! - Good Display product page: <https://www.good-display.com/product/412.html>
//! - Waveshare product page: <https://www.waveshare.com/2.66inch-e-Paper.htm>
//! - Waveshare wiki: <http://www.waveshare.com/wiki/2.66inch_e-Paper_Module>
//! - Waveshare manual: <http://www.waveshare.com/wiki/2.66inch_e-Paper_Module_Manual>
//! - Datasheet: <https://files.waveshare.com/upload/d/dc/2.66inch-e-paper-specification.pdf>
//! - Waveshare reference driver: <https://github.com/waveshareteam/e-Paper/blob/master/RaspberryPi_JetsonNano/c/lib/e-Paper/EPD_2in66.c>
//! - GxEPD2 reference driver: <https://github.com/ZinggJM/GxEPD2/blob/master/src/gdey/GxEPD2_266_GDEY0266T90.h>
//! - Adafruit_EPD reference driver (Gray4 only, not a Good Display/Waveshare source):
//!   <https://github.com/adafruit/Adafruit_EPD/blob/master/src/panels/ThinkInk_266_Grayscale4_MFGN.h>

use crate::traits::{ColorMode, EpdPanel, Gray4Registers};

/// 4-level grayscale waveform LUT for [`GDEY0266T90`], transcribed verbatim from Adafruit_EPD's
/// `ti_266mfgn_gray4_lut_code` (233 bytes — longer than a standard mono LUT; see the module doc
/// and [`Gray4Registers`] for why).
const GRAY4_LUT: [u8; 233] = [
    0x00, 0x60, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x20, 0x60, 0x10, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x28, 0x60, 0x14, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x2A, 0x60, 0x15, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x90, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x05,
    0x14, 0x00, 0x00, 0x1E, 0x1E, 0x00, 0x00, 0x00, 0x00, 0x01, 0x00, 0x02, 0x00, 0x05, 0x14, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x24, 0x22, 0x22, 0x22, 0x23, 0x32, 0x00, 0x00, 0x00,
];

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

    /// 4-level grayscale register bundle. See the module doc's "4-level grayscale" section for
    /// provenance — this is from Adafruit_EPD, not Good Display/Waveshare, and opt-in only (pass
    /// explicitly via `.with_gray4(GDEY0266T90::GRAY4)`, not read automatically by `for_panel`).
    const GRAY4: Option<Gray4Registers> = Some(Gray4Registers {
        vcom: 0x28,
        gate_voltage: 0x17,
        source_voltage: [0x41, 0xAE, 0x32],
        border_waveform: 0x04,
        lut_end_option: 0x22,
        lut: &GRAY4_LUT,
    });
}

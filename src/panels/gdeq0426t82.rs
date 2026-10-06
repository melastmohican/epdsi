//! Panel specification for GDEQ0426T82 (4.26" 800x480 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Seeed Hardware**: Seeed Studio Product 6398 (4.26" Monochrome SPI ePaper Display)
//! - **Controller IC**: SE8350 / SSD1677. "SE8350" is the part number in the panel maker's
//!   datasheet (YES-0347-Y03, hosted by Seeed) and on Seeed's product page, which lists it as
//!   `SE8350 (SSD1677)`. The SE8350 IC spec itself is not public, so the SSD1677 command set is
//!   what this crate drives, verified on hardware.
//! - **Native Resolution**: 800 x 480 pixels (already byte-aligned, no RAM padding)
//! - **Y-Axis Reversal**: panel gates are physically wired in reverse; `Ssd1677Controller`
//!   compensates for this in software (see its `set_window`/`set_cursor` implementation), so
//!   this is transparent to callers.
//! - **Busy Polarity**: Active-HIGH (busy while HIGH).
//! - **4-level grayscale (Gray4)**: **not** Seeed/Good Display material — Seeed's own spec lists
//!   this as a 2-level (monochrome) panel. It is transcribed verbatim from Adafruit_EPD's
//!   `ThinkInk_426_Grayscale4_GDEQ` reference driver (`ti_426_gray4_init_code` /
//!   `ti_426_gray4_lut_code`), the only Gray4 reference for this controller/panel pairing.
//!   **Confirmed on physical hardware** on all four boards this crate targets — RP2350 (blocking
//!   and async), ESP32-C3, and RP2040 — each rendering four distinct gray levels correctly.
//!
//!   Unlike [`GDEY0266T90`](crate::panels::GDEY0266T90)'s single-pass SSD1680 Gray4 mode,
//!   `Adafruit_SSD1677::update()`'s grayscale branch is **two-pass**: a full refresh with the OTP
//!   LUT (Red/Yellow plane bypassed) to set a known monochrome baseline, then the custom LUT and
//!   voltage registers are reloaded, then a second refresh with the real Black/White (LSB) and
//!   Red/Yellow (MSB) planes. Use
//!   [`Ssd1677Controller::for_panel::<GDEQ0426T82>().with_gray4(GDEQ0426T82::GRAY4)`](crate::controllers::Ssd1677Controller::with_gray4)
//!   together with [`Ssd1677RefreshMode::Gray4Preclear`](crate::controllers::Ssd1677RefreshMode::Gray4Preclear)/[`Gray4`](crate::controllers::Ssd1677RefreshMode::Gray4)
//!   and [`Ssd1677Controller::reload_gray4_lut`](crate::controllers::Ssd1677Controller::reload_gray4_lut)
//!   — see the hardware examples (`ssd1677_gdeq0426t82_gray4_epd` in this crate's example repos)
//!   for the full two-pass call sequence. The generic `render_paged_gray4` (single-pass) will not
//!   drive this correctly; [`render_paged_gray4_preclear`](crate::graphics::paged::render_paged_gray4_preclear)
//!   exists for a paged/bounded-memory caller, but the confirmed examples instead hold the whole
//!   800x480 frame in two static buffers and call the primitives directly, since that fits
//!   comfortably on every board this crate targets.
//!
//! ### Vendor References
//! - Adafruit_EPD reference driver (Gray4 only, not a Seeed/Good Display source):
//!   <https://github.com/adafruit/Adafruit_EPD/blob/master/src/panels/ThinkInk_426_Grayscale4_GDEQ.h>
//! - Adafruit_EPD SSD1677 driver (two-pass grayscale `update()`):
//!   <https://github.com/adafruit/Adafruit_EPD/blob/master/src/drivers/Adafruit_SSD1677.cpp>

use crate::traits::{ColorMode, EpdPanel, Gray4Registers};

/// 4-level grayscale waveform LUT for [`GDEQ0426T82`] (105 bytes), transcribed verbatim from
/// Adafruit_EPD's `ti_426_gray4_lut_code`: 5 VS-waveform groups of 10 bytes each (white, light
/// gray, dark gray, black, L4/VCOM), then 10 TP/RP groups of 5 bytes, then a 5-byte frame rate
/// tail. See the module doc's "4-level grayscale" section for provenance and the two-pass
/// sequence this LUT is used with.
const GRAY4_LUT: [u8; 105] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x54, 0x54, 0x40, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0xAA, 0xA0, 0xA8, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xA2, 0x22,
    0x20, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x01, 0x01, 0x01, 0x01, 0x00, 0x01, 0x01, 0x01, 0x01, 0x00, 0x01, 0x01, 0x01, 0x01,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x8F, 0x8F, 0x8F, 0x8F, 0x8F,
];

/// Physical panel driver specification for Seeed Studio 6398 4.26" Monochrome ePaper (SE8350/SSD1677, GxEPD2 `GDEQ0426T82`).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEQ0426T82;

impl EpdPanel for GDEQ0426T82 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 800;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 480;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;

    /// 4-level grayscale register bundle. See the module doc's "4-level grayscale" section for
    /// provenance — this is from Adafruit_EPD, not Good Display/Seeed material, and opt-in only
    /// (pass explicitly via `.with_gray4(GDEQ0426T82::GRAY4)`, not read automatically by
    /// `for_panel`). Drive it with
    /// [`render_paged_gray4_preclear`](crate::graphics::paged::render_paged_gray4_preclear), not
    /// the generic `render_paged_gray4` — this controller's Gray4 sequence is two-pass, see
    /// [`Ssd1677RefreshMode::Gray4Preclear`](crate::controllers::Ssd1677RefreshMode::Gray4Preclear).
    ///
    /// Use [`Gray4Polarity::ADAFRUIT_SSD1677`](crate::graphics::buffer::Gray4Polarity::ADAFRUIT_SSD1677)
    /// (not `ADAFRUIT_SSD1680`) when drawing — this panel's two RAM planes are both inverted,
    /// unlike `GDEY0266T90`'s.
    const GRAY4: Option<Gray4Registers> = Some(Gray4Registers {
        vcom: 0x30,
        gate_voltage: 0x17,
        source_voltage: [0x41, 0xA8, 0x32],
        border_waveform: 0x01,
        // Inert for this controller: the SSD1677 has no "Option for LUT end" (0x3F) register in
        // its Gray4 path — `Ssd1677Controller` never reads this field. Kept only because
        // `Gray4Registers` is a shared bundle across controller families (see `Ssd168xController`,
        // which does use it).
        lut_end_option: 0x00,
        lut: &GRAY4_LUT,
    });
}

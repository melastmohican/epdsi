//! Panel specification for GDEM0213B74 (2.13" 122x250 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Adafruit Hardware**: Adafruit Product ID 6383 (Adafruit ThinkInk 2.13" Monochrome display breakout)
//! - **Controller IC**: SSD1680Z
//! - **Native Resolution**: 122 x 250 pixels (visible)
//! - **RAM Alignment**: 128 pixels (16 bytes per row) in SSD1680 RAM layout — handled automatically by
//!   the controller's byte-boundary window/cursor addressing, no special panel-side padding needed.
//! - **Busy Polarity**: Active-HIGH (busy while HIGH).
//! - **4-level grayscale (Gray4)**: `GRAY4` on this panel is **not** Adafruit's own product-page
//!   material for this breakout's default monochrome mode. It's transcribed verbatim from
//!   Adafruit_EPD's `ThinkInk_213_Grayscale4_MFGN` reference driver
//!   (`ti_213mfgn_gray4_init_code`/`ti_213mfgn_gray4_lut_code`), and is byte-identical to the
//!   already-shipped [`GDEY0266T90`](crate::panels::GDEY0266T90)'s `GRAY4` bundle (same 233-byte
//!   LUT, same VCOM/gate/source/LUT-end bytes): both come from the same Adafruit reference
//!   family for the SSD1680 IC. Use it via
//!   `Ssd1680Controller::for_panel::<GDEM0213B74>().with_gray4(GDEM0213B74::GRAY4)` together with
//!   [`Ssd168xRefreshMode::Gray4`](crate::controllers::Ssd168xRefreshMode::Gray4).
//!   Adafruit's own driver carries a per-breakout-revision `_colstart` offset (0 by default, 8 for
//!   the newer FPC-7528B revision) that `epdsi` has no equivalent of in either Gray4 or plain mono
//!   mode today. If a Gray4 image renders shifted on your board, check whether your unit is that
//!   revision before suspecting the register bundle itself.
//!
//! ### Vendor References
//! - Adafruit_EPD reference driver (Gray4 only, not Adafruit's product-page default mode):
//!   <https://github.com/adafruit/Adafruit_EPD/blob/master/src/panels/ThinkInk_213_Grayscale4_MFGN.h>

use crate::traits::{ColorMode, EpdPanel, Gray4Registers};

/// 4-level grayscale waveform LUT for [`GDEM0213B74`], transcribed verbatim from Adafruit_EPD's
/// `ti_213mfgn_gray4_lut_code`. Byte-identical to [`GDEY0266T90`](crate::panels::GDEY0266T90)'s
/// `GRAY4_LUT`, confirmed by direct comparison against Adafruit's source rather than assumed from
/// the shared reference family.
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

/// Physical panel driver specification for Adafruit 6383 2.13" Monochrome ePaper (SSD1680Z, GDEM0213B74).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEM0213B74;

/// GxEPD2 reference alias for this panel (`GxEPD2_213_B74`).
#[allow(non_camel_case_types)]
pub type GxEPD2_213_B74 = GDEM0213B74;

impl EpdPanel for GDEM0213B74 {
    /// Panel visible width in pixels.
    const WIDTH: u32 = 122;

    /// Panel visible height in pixels.
    const HEIGHT: u32 = 250;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;

    /// 4-level grayscale register bundle. See the module doc's "4-level grayscale" section for
    /// provenance: this is from Adafruit_EPD, not a Good Display/Waveshare source, and opt-in
    /// only (pass explicitly via `.with_gray4(GDEM0213B74::GRAY4)`, not read automatically by
    /// `for_panel`).
    const GRAY4: Option<Gray4Registers> = Some(Gray4Registers {
        vcom: 0x28,
        gate_voltage: 0x17,
        source_voltage: [0x41, 0xAE, 0x32],
        border_waveform: 0x04,
        lut_end_option: 0x22,
        lut: &GRAY4_LUT,
    });
}

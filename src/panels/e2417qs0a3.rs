//! Panel specification for Pervasive Displays E2417QS0A3 (4.2" Spectra-4/BWRY e-Paper display).
//!
//! ### Hardware Notes:
//! - **Reference Driver**: `Pervasive_BWRY_Small` (`eScreen_EPD_417_QS_0A`, Driver A)
//! - **Native Resolution**: 400 x 300 pixels (no RAM over-scan: `30000` bytes at 2 bits-per-pixel
//!   `= 400*300*2/8`, exactly matching the visible pixel count).
//! - **Color Mode**: Spectra-4 (Black, White, Red, Yellow), 2 bits-per-pixel packed.
//! - **Busy Polarity**: Active-low (busy while LOW).

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for Pervasive Displays E2417QS0A3 (BWRY Driver A).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct E2417QS0A3;

/// Pervasive reference alias for 4.2" QS Driver A display (`eScreen_EPD_417_QS_0A` / `417-QS-0A`).
#[allow(non_camel_case_types)]
pub type EPD_417_QS_0A = E2417QS0A3;

impl EpdPanel for E2417QS0A3 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 400;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 300;

    /// Display color operating mode (Spectra-4: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;
}

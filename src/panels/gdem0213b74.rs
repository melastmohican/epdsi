//! Panel specification for GDEM0213B74 (2.13" 122x250 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Adafruit Hardware**: Adafruit Product ID 6383 (Adafruit ThinkInk 2.13" Monochrome display breakout)
//! - **Controller IC**: SSD1680Z
//! - **Native Resolution**: 122 x 250 pixels (visible)
//! - **RAM Alignment**: 128 pixels (16 bytes per row) in SSD1680 RAM layout — handled automatically by
//!   the controller's byte-boundary window/cursor addressing, no special panel-side padding needed.
//! - **Busy Polarity**: Active-HIGH (busy while HIGH).

use crate::traits::{ColorMode, EpdPanel};

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
}

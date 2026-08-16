//! Panel specification for GDEQ0426T82 (4.26" 800x480 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Seeed Hardware**: Seeed Studio Product 6398 (4.26" Monochrome SPI ePaper Display)
//! - **Controller IC**: SE8350 / SSD1677
//! - **Native Resolution**: 800 x 480 pixels (already byte-aligned, no RAM padding)
//! - **Y-Axis Reversal**: panel gates are physically wired in reverse; `Ssd1677Controller`
//!   compensates for this in software (see its `set_window`/`set_cursor` implementation), so
//!   this is transparent to callers.
//! - **Busy Polarity**: Active-HIGH (busy while HIGH).

use crate::traits::{ColorMode, EpdPanel};

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
}

//! Panel specification for GDEY037T03 (3.7" 240x416 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Adafruit Hardware**: Adafruit Product ID 6395 (Adafruit ThinkInk 3.7" Monochrome display breakout)
//! - **Controller IC**: UC8253
//! - **Native Resolution**: 240 x 416 pixels (already byte-aligned, no RAM padding)
//! - **Busy Polarity**: Active-**LOW** (busy while LOW) — opposite of the SSD16xx-family panels
//!   in this crate.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for Adafruit 6395 3.7" Monochrome ePaper (UC8253, GxEPD2 `GDEY037T03`).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEY037T03;

/// GxEPD2 reference alias for this panel (`GxEPD2_370_GDEY037T03`).
#[allow(non_camel_case_types)]
pub type GxEPD2_370_GDEY037T03 = GDEY037T03;

impl EpdPanel for GDEY037T03 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 240;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 416;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}

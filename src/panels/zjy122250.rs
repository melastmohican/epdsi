//! Panel specification for ZJY122250-0213AJH-E5 (2.13" 250x122 Mono/Tri-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Panel Model**: `ZJY122250-0213AJH-E5` (FPC ribbon cable model stamp)
//! - **Good Display Equivalent**: `GDEY0213F51` (referenced in GxEPD2 library as `GxEPD2_213c_GDEY0213F51`)
//! - **Adafruit Hardware**: Used in Adafruit Product IDs 6373 and 6366 (Adafruit ThinkInk 2.13" display breakout)
//! - **Controller IC**: JD79661
//! - **Resolution**: 250 x 122 pixels

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for ZJY122250-0213AJH-E5 / Good Display GDEY0213F51 (Adafruit 6373/6366).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZJY122250_0213AJH_E5;

/// Type alias for Good Display part number `GDEY0213F51`.
pub type GDEY0213F51 = ZJY122250_0213AJH_E5;

impl EpdPanel for ZJY122250_0213AJH_E5 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 250;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 122;

    /// Panel color operating mode.
    const COLOR_MODE: ColorMode = ColorMode::TriColor;
}

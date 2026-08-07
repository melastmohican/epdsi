//! Panel specification for ZJY122250-0213AJH-E5 (2.13" 250x122 Mono/Tri-Color e-Paper display).

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for ZJY122250-0213AJH-E5.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZJY122250_0213AJH_E5;

impl EpdPanel for ZJY122250_0213AJH_E5 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 250;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 122;

    /// Panel color operating mode.
    const COLOR_MODE: ColorMode = ColorMode::TriColor;
}

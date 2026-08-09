//! Panel specification for Good Display GDEP073E01 (7.3" 800x480 7-Color ACeP display panel).

use crate::traits::{ColorMode, EpdPanel};

/// Hardware specification for Good Display GDEP073E01 7.3-inch 7-Color ACeP / Spectra 6 E-Paper Display Panel.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEP073E01;

/// GxEPD2 reference alias for 7.3" 7-Color display (`GxEPD2_730c_GDEP073E01`).
#[allow(non_camel_case_types)]
pub type GxEPD2_730c_GDEP073E01 = GDEP073E01;

impl EpdPanel for GDEP073E01 {
    /// Physical width of the display panel in pixels.
    const WIDTH: u32 = 800;

    /// Physical height of the display panel in pixels.
    const HEIGHT: u32 = 480;

    /// Display color operating mode (7-Color ACeP / Spectra 6).
    const COLOR_MODE: ColorMode = ColorMode::SevenColor;
}

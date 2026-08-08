//! Panel specification for Pervasive Displays E2266KS0C1 (2.66" 296x152 Monochrome display panel).

use crate::traits::{ColorMode, EpdPanel};

/// Hardware specification for Pervasive Displays E2266KS0C1 2.66-inch Monochrome E-Paper Display Panel.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct E2266KS0C1;

impl EpdPanel for E2266KS0C1 {
    /// Physical width of the display panel in pixels.
    const WIDTH: u32 = 152;

    /// Physical height of the display panel in pixels.
    const HEIGHT: u32 = 296;

    /// Display color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}

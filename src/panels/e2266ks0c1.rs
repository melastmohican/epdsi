//! Panel specification for Pervasive Displays E2266KS0C1 (2.66" 296x152 Monochrome display panel).
//!
//! ### Vendor References
//! - [Product page](https://www.pervasivedisplays.com/products/2-66-e-ink-displays/)
//! - [Datasheet flyer (PDF)](https://www.pervasivedisplays.com/wp-content/uploads/2025/10/Flyer_E2266KS0C1_20241209.pdf) —
//!   confirms resolution `296(H) x 152(V) pixel`, active area 60.088 × 30.704 mm.

use crate::traits::{ColorMode, EpdPanel};

/// Hardware specification for Pervasive Displays E2266KS0C1 2.66-inch Monochrome E-Paper Display Panel.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct E2266KS0C1;

/// Pervasive reference alias for 2.66" K-film Driver C display (`EPD_266_KS_0C` / `266-KS-0C`).
#[allow(non_camel_case_types)]
pub type EPD_266_KS_0C = E2266KS0C1;

impl EpdPanel for E2266KS0C1 {
    /// Physical width of the display panel in pixels.
    const WIDTH: u32 = 152;

    /// Physical height of the display panel in pixels.
    const HEIGHT: u32 = 296;

    /// Display color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}

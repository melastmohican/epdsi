//! Panel specification for Pervasive Displays E2290KS0F1 (2.9" 384x168 Monochrome display panel).
//!
//! ### Vendor References
//! - [Product page](https://www.pervasivedisplays.com/products/2-9-e-ink-displays/)
//! - [Datasheet flyer (PDF)](https://www.pervasivedisplays.com/wp-content/uploads/2025/10/Flyer_E2290KS0F1_20241210.pdf) —
//!   confirms resolution `384(H) x 168(V) pixel`, active area 67.584 × 29.568 mm.

use crate::traits::{ColorMode, EpdPanel};

/// Hardware specification for Pervasive Displays E2290KS0F1 2.9-inch Monochrome E-Paper Display Panel.
///
/// ### EXT3-1 Extension Board Jumper Note:
/// Ensure the **J3 jumper** on the EXT3-1 extension board is **OPEN** (selecting the 10 µH inductor path).
/// If J3 is closed (47 µH path for large screens), the DC-DC boost converter chokes during refresh,
/// causing voltage sag and initialization / busy-wait hangs.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct E2290KS0F1;

/// Pervasive reference alias for 2.90" K-film Driver F display (`EPD_290_KS_0F` / `290-KS-0F`).
#[allow(non_camel_case_types)]
pub type EPD_290_KS_0F = E2290KS0F1;

impl EpdPanel for E2290KS0F1 {
    /// Physical width of the display panel in pixels.
    const WIDTH: u32 = 168;

    /// Physical height of the display panel in pixels.
    const HEIGHT: u32 = 384;

    /// Display color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}

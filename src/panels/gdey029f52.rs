//! Panel specification for GDEY029F52 (2.9" 128x296 Quad-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Panel Model**: `GDEY029F52` ([Product Page](https://www.good-display.com/product/742.html)),
//!   rear sticker `029SW-F52-B2`. Good Display lists the raster as 296 x 128. The IC sees it as
//!   128 pixels per RAM row and 296 rows, which is what `WIDTH` and `HEIGHT` hold here.
//! - **Not the `GDEY029F51H`**: that is a different Good Display product (JD79667, 168 x 384,
//!   [product page](https://www.good-display.com/product/464.html)). A unit ordered as an F51H
//!   arrived with the `029SW-F52-B2` sticker and drew correctly only under the 128 x 296 init
//!   (the 168 x 384 init timed out at refresh), so check the rear sticker before choosing a panel.
//! - **Controller IC**: JD79661. Use [`Jd79661Controller`](crate::controllers::Jd79661Controller).
//!   The controller never sees the panel type, so pairing this panel with another variant
//!   gives a wrong or blank picture, not a compile error.
//! - **Observed `R70H` revision bytes**: `83 02 01` on a Good Display unit (lot `N2B3F02`).
//!   The JD79661AA datasheet lists `03 02 01`, so the value matches once the top bit is
//!   cleared. The same top bit is set on the `GDEY0213F52` unit from Good Display.
//! - **Init**: Good Display's own demo sends PSR `0F 29` and the `Jd79661` variant sends `8F 29`.
//!   Both, and `4F 29`, drew the same picture on a real unit, so TRES sets the geometry and the
//!   PSR resolution bits do not matter here.
//! - **Native Resolution**: 128 x 296 pixels
//! - **RAM Alignment**: 128 pixels (32 bytes per row), 9,472 bytes per 2bpp Quad-Color frame.
//! - **Refresh**: Good Display lists 16 s for a full refresh and 11 s for its fast mode. The
//!   fast init is a different register sequence and is not implemented. BUSY is active-low.
//! - **Colour codes**: black `00`, white `01`, yellow `10`, red `11`.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel specification for Good Display `GDEY029F52` (JD79661).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEY029F52;

impl EpdPanel for GDEY029F52 {
    /// Panel physical width in pixels (RAM row length).
    const WIDTH: u32 = 128;

    /// Panel physical height in pixels (number of RAM rows).
    const HEIGHT: u32 = 296;

    /// Panel color operating mode (Quad-Color: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;
}

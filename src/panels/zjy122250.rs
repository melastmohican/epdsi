//! Panel specification for ZJY122250-0213AJH-E5 (2.13" 122x250 Quad-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Panel Model**: `ZJY122250-0213AJH-E5` (FPC ribbon cable model stamp)
//! - **Good Display Equivalent**: `GDEY0213F51` ([Product Page](https://www.good-display.com/product/463.html), referenced in GxEPD2 library under `GxEPD2_4C` 4-color family as `GxEPD2_213c_GDEY0213F51`)
//! - **Seeed Studio Hardware**: [2.13" Quadruple Color ePaper Display (122x250)](https://www.seeedstudio.com/2-13-Quadruple-Color-ePaper-Display-with-122x250-Pixels-p-5779.html) (SKU: 104990666)
//! - **Adafruit Hardware**: Used in Adafruit Product IDs [6373](https://www.adafruit.com/product/6373) and [6366](https://www.adafruit.com/product/6366) (Adafruit ThinkInk 2.13" Quad-Color display breakout: Black, White, Red, Yellow)
//! - **Identifying an unlabelled unit**: the flex ribbon is stamped `FPC-J002` followed
//!   by a batch date code (e.g. `22.02.28`). Units sold by Adafruit and Seeed under
//!   their own part numbers and stickers carry the same `FPC-J002` ribbon and are
//!   physically identical, so the ribbon stamp identifies the panel where the retail
//!   labelling does not.
//! - **Controller IC**: JD79661
//! - **Native Resolution**: 122 x 250 pixels
//! - **RAM Alignment**: 128 pixels (32 bytes per row) in JD79661 RAM layout (8,000 bytes total per 2bpp QuadColor frame).

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for ZJY122250-0213AJH-E5 / Good Display [GDEY0213F51](https://www.good-display.com/product/463.html) / Seeed Studio [5779](https://www.seeedstudio.com/2-13-Quadruple-Color-ePaper-Display-with-122x250-Pixels-p-5779.html) / Adafruit [6373](https://www.adafruit.com/product/6373)/[6366](https://www.adafruit.com/product/6366).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZJY122250_0213AJH_E5;

/// Type alias for Good Display 4-color part number `GDEY0213F51`.
pub type GDEY0213F51 = ZJY122250_0213AJH_E5;

impl EpdPanel for ZJY122250_0213AJH_E5 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 122;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 250;

    /// Panel color operating mode (Quad-Color: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;
}

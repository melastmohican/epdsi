//! Panel specification for Pervasive Displays E2154QS0F1 (1.54" Spectra-4/BWRY e-Paper display).
//!
//! ### Hardware Notes:
//! - **Reference Driver**: `Pervasive_BWRY_Small` (`eScreen_EPD_152_QS_06`, Driver 6)
//! - **Visible Resolution**: 152 x 152 pixels
//! - **RAM Buffer**: the underlying COG RAM buffer is over-scanned to 200 x 200 pixels
//!   (`10000` bytes at 2 bits-per-pixel `= 200*200*2/8`) — only the top-left 152x152 region is
//!   visible on the physical panel. `WIDTH`/`HEIGHT` below reflect the RAM/buffer dimensions
//!   (matching the existing precedent set by SSD1680's 122->128 byte-padding), not the visible
//!   pixel count.
//! - **Color Mode**: Spectra-4 (Black, White, Red, Yellow), 2 bits-per-pixel packed.
//! - **Busy Polarity**: Active-low (busy while LOW).

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for Pervasive Displays E2154QS0F1 (BWRY Driver 6).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct E2154QS0F1;

/// Pervasive reference alias for 1.54" QS Driver 6 display (`eScreen_EPD_152_QS_06` / `152-QS-06`).
#[allow(non_camel_case_types)]
pub type EPD_152_QS_06 = E2154QS0F1;

impl EpdPanel for E2154QS0F1 {
    /// RAM buffer width in pixels (200x200 over-scanned COG buffer; visible area is 152x152).
    const WIDTH: u32 = 200;

    /// RAM buffer height in pixels (200x200 over-scanned COG buffer; visible area is 152x152).
    const HEIGHT: u32 = 200;

    /// Display color operating mode (Spectra-4: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;
}

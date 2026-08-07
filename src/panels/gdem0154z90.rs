//! Hardware specification for GDEM0154Z90 (1.54" 200x200 Tri-Color display panel).

use crate::traits::{ColorMode, EpdPanel};

/// GDEM0154Z90 1.54-inch Tri-Color (Black, White, Red) E-Paper Display Panel.
#[derive(Debug, Clone, Copy, Default)]
pub struct GDEM0154Z90;

impl EpdPanel for GDEM0154Z90 {
    const WIDTH: u32 = 200;
    const HEIGHT: u32 = 200;
    const COLOR_MODE: ColorMode = ColorMode::TriColor;

    fn vcom(&self) -> Option<u8> {
        Some(0x26)
    }
}

//! Hardware specification for GDEM0154Z90 (1.54" 200x200 Tri-Color display panel).

use crate::traits::{ColorMode, EpdPanel};

/// GDEM0154Z90 1.54-inch Tri-Color (Black, White, Red) E-Paper Display Panel.
///
/// # No VCOM override
///
/// Before 0.1.6 this panel declared `vcom() -> Some(0x26)`. That hook was never reachable, so
/// the value never left the crate — and it should not: `GxEPD2_154_Z90c::_InitDisplay()`, the
/// reference driver for this panel, writes no `0x2C` at all. It runs on the panel's OTP VCOM.
///
/// `0x26` is the VCOM that `GxEPD2_213_B72::_Init_Part()` writes for a *different* panel on a
/// *different* IC in partial mode. Carrying it over to the const, now that the const actually
/// reaches the wire, would have turned a dead placeholder into a live divergence. Ruled
/// 1 Sep 2026 against `GxEPD2_154_Z90c.cpp`; see `gxepd2-parity-audit`.
#[derive(Debug, Clone, Copy, Default)]
pub struct GDEM0154Z90;

impl EpdPanel for GDEM0154Z90 {
    const WIDTH: u32 = 200;
    const HEIGHT: u32 = 200;
    const COLOR_MODE: ColorMode = ColorMode::TriColor;
}

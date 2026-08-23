//! Panel specification for Good Display GDEP073E01 (7.3" 800x480 E Ink Spectra 6 display panel).

use crate::traits::{ColorMode, EpdPanel};

/// Hardware specification for Good Display GDEP073E01, a 7.3-inch 800x480 E-Paper panel.
///
/// This is an **E Ink Spectra 6 (E6)** panel — the vendor part number is
/// `GDEP073E01(E6)`. It renders six colours: black, white, red, yellow, blue and
/// green. It is *not* a 7-colour ACeP panel, despite using the same 4-bit
/// [`SevenColor`](crate::traits::SevenColor) palette encoding.
///
/// [`SevenColor::Orange`](crate::traits::SevenColor::Orange) is therefore **not
/// renderable here** — it belongs to the older ACeP-7 generation. Sending it
/// produces an undefined colour rather than orange.
/// [`SevenColor::Clean`](crate::traits::SevenColor::Clean) renders as white.
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEP073E01;

/// GxEPD2 reference alias for the 7.3" Spectra 6 display (`GxEPD2_730c_GDEP073E01`).
#[allow(non_camel_case_types)]
pub type GxEPD2_730c_GDEP073E01 = GDEP073E01;

impl EpdPanel for GDEP073E01 {
    /// Physical width of the display panel in pixels.
    const WIDTH: u32 = 800;

    /// Physical height of the display panel in pixels.
    const HEIGHT: u32 = 480;

    /// Display color operating mode (4 bpp ACeP / Spectra palette).
    const COLOR_MODE: ColorMode = ColorMode::SevenColor;
}

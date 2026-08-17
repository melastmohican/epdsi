//! Panel specification for Pervasive Displays E2154QS0F1 (1.54" Spectra-4/BWRY e-Paper display).
//!
//! ### Hardware Notes:
//! - **Reference Driver**: `Pervasive_BWRY_Small` (`eScreen_EPD_154_QS_0F`, Driver F)
//! - **Native Resolution**: 152 x 152 pixels (no RAM over-scan: `5776` bytes at 2 bits-per-pixel
//!   `= 152*152*2/8`, matching `PDLS_Common`'s `frameSize_EPD_154` constant exactly).
//! - **Color Mode**: Spectra-4 (Black, White, Red, Yellow), 2 bits-per-pixel packed.
//! - **Busy Polarity**: Active-low (busy while LOW).
//!
//! ### Vendor References
//! - [Product page](https://www.pervasivedisplays.com/products/1-54-e-ink-displays/#spectra-4) —
//!   confirms Screen/Driver Code `154-QS-0F1`, resolution 152 × 152 pixels.
//! - [Datasheet flyer (PDF)](https://www.pervasivedisplays.com/wp-content/uploads/2025/10/Flyer_E2154QS0F1_20241022.pdf) —
//!   confirms resolution `152(H) x 152(V) pixel`, active area 27.512 × 27.512 mm.
//!
//! These two sources are what caught and confirmed the fix for an earlier implementation that
//! targeted the wrong Pervasive screen code (`152-QS-06` instead of `154-QS-0F1`) — see the
//! `pervasive-parity-audit` skill's "Panel Identification Verification" section for why this
//! class of bug isn't caught by register-level parity checks alone.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for Pervasive Displays E2154QS0F1 (BWRY Driver F).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct E2154QS0F1;

/// Pervasive reference alias for 1.54" QS Driver F display (`eScreen_EPD_154_QS_0F` / `154-QS-0F`).
#[allow(non_camel_case_types)]
pub type EPD_154_QS_0F = E2154QS0F1;

impl EpdPanel for E2154QS0F1 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 152;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 152;

    /// Display color operating mode (Spectra-4: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;
}

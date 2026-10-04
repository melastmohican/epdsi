//! Panel specification for GDEY037T03 (3.7" 240x416 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Adafruit Hardware**: Adafruit Product ID 6395 (Adafruit ThinkInk 3.7" Monochrome display breakout)
//! - **Controller IC**: UC8253
//! - **Native Resolution**: 240 x 416 pixels (already byte-aligned, no RAM padding)
//! - **Busy Polarity**: Active-**LOW** (busy while LOW) — opposite of the SSD16xx-family panels
//!   in this crate.
//! - **Status**: register sequence (soft-reset toggle during init, then a per-refresh `CDI`
//!   re-issue, `0x97` full / `0xD7` partial) matches the GxEPD2 reference driver
//!   (`GxEPD2_370_GDEY037T03.cpp`) byte-for-byte, confirmed directly against GxEPD2's current
//!   source, not assumed. Hardware-verified blocking on RP2350, RP2040 and ESP32-C3, and async
//!   on RP2350 (see the README's "Examples on real hardware" table and the `CHANGELOG.md` 0.2.0
//!   entry, which separately names this panel as running cleanly on the replacement ESP32-C3
//!   module).
//! - **A real, benign register divergence from other references for this same board exists.**
//!   Adafruit's own `ThinkInk_370_Mono_BAAMFGN.h`, for this exact Adafruit Product ID 6395
//!   board, uses a structurally different flow: no soft-reset toggle, one `CDI`+`PANEL_SETTING`
//!   write at init, `CDI` never re-issued per refresh. Good Display's own vendor demo for this
//!   panel differs again: no `PANEL_SETTING` write at all, separate init functions per refresh
//!   mode instead of a shared per-refresh `CDI` write. All three are real, independently
//!   sourced flows for the same silicon. `epdsi` follows GxEPD2's because it's the one that's
//!   actually been bench-confirmed here, not because it's closest to any single reference; which
//!   byte set this panel's own OTP calibration was tuned against is unconfirmed.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for Adafruit 6395 3.7" Monochrome ePaper (UC8253, GxEPD2 `GDEY037T03`).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEY037T03;

/// GxEPD2 reference alias for this panel (`GxEPD2_370_GDEY037T03`).
#[allow(non_camel_case_types)]
pub type GxEPD2_370_GDEY037T03 = GDEY037T03;

impl EpdPanel for GDEY037T03 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 240;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 416;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}

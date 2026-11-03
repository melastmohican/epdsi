//! Panel specification for GDEW0102I4FC (1.02" 80x128 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor**: Good Display.
//! - **Controller IC**: UC8175, driven by [`Uc8175Controller`](crate::controllers::Uc8175Controller).
//!   Other UC81xx controllers in this crate use a different init and picking one of them renders
//!   blank or wrong instead of erroring.
//! - **Native resolution**: 80 x 128 pixels (source by gate), already byte-aligned.
//! - **Busy polarity**: Active-**LOW** (busy while LOW). Use a pull-up input.
//! - **Connector**: 30-pin 0.5 mm FPC (marking `WFT0102BF37`). It does not fit the 24-pin
//!   `DESPI-C02`; use the `DESPI-C102` adapter.
//! - **Waveform**: stored in the on-chip OTP, 1-bit. Full refresh only (about 3 s), no partial
//!   refresh and no gray levels.
//! - **Status**: register sequence is Good Display's own `AU-GDEW0102I4FC-20250926` demo, checked
//!   against `GDEW0102I4FC.pdf`. Bench verified on 2 Nov 2026 with the blocking API on an XIAO ESP32-C3 wired by hand to a `DESPI-C102` adapter: the full refresh rendered the Rust and Ferris picture.

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel specification for the Good Display 1.02" Monochrome ePaper (UC8175).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEW0102I4FC;

impl EpdPanel for GDEW0102I4FC {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 80;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 128;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}

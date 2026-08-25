//! Panel specification for GDEM0154F51H (1.54" 200×200 Quad-Color e-Paper).
//!
//! ### Hardware Notes
//! - **Good Display**: [`GDEM0154F51H`](https://www.good-display.com/product/555.html)
//!   (GxEPD2 class `GxEPD2_154c_GDEM0154F51H`)
//! - **Controller IC**: JD79660
//! - **Native Resolution**: 200 × 200 pixels (already 4-pixel / 8-pixel aligned)
//! - **Color**: Quad-Color 2 bpp — `00` Black, `01` White, `10` Yellow, `11` Red
//!   (4 pixels per byte, 10 000 bytes per full frame)
//! - **Busy Polarity**: Active-**LOW** (busy while LOW, idle while HIGH)
//! - **Refresh**: Full refresh only in practice (~20–25 s). GxEPD2 reports
//!   `hasPartialUpdate = true` with `hasFastPartialUpdate = false`; a windowed
//!   write still costs a full waveform.
//! - **Waveshare board**: [ESP32-S3-ePaper-1.54G](https://www.waveshare.com/esp32-s3-epaper-1.54g.htm)
//!   SKU **34586**. Panel power enable `EPD3V3_EN` (GPIO6, **active-low**) is
//!   board bring-up, not part of this crate — drive it LOW before `init`.
//!
//! [`EpdDriver::clear_frame`](crate::driver::EpdDriver::clear_frame) sizes the
//! fill as 1 bpp (`width.div_ceil(8) * height`). For this panel send a packed
//! 2 bpp buffer with [`write_frame`](crate::driver::EpdDriver::write_frame)
//! ([`GDEM0154F51H::FRAME_BYTES`] bytes of `0x55` for white).

use crate::traits::{ColorMode, EpdPanel};

/// Good Display [GDEM0154F51H](https://www.good-display.com/product/555.html)
/// 1.54" 200×200 Quad-Color panel (JD79660).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEM0154F51H;

/// GxEPD2 reference alias (`GxEPD2_154c_GDEM0154F51H`).
#[allow(non_camel_case_types)]
pub type GxEPD2_154c_GDEM0154F51H = GDEM0154F51H;

impl EpdPanel for GDEM0154F51H {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 200;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 200;

    /// Quad-Color: Black, White, Yellow, Red (2 bpp).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;
}

impl GDEM0154F51H {
    /// Packed 2 bpp frame size (`WIDTH * HEIGHT / 4`).
    pub const FRAME_BYTES: usize = (Self::WIDTH as usize * Self::HEIGHT as usize) / 4;
}

//! Panel specification for SE0352N14-TNG-A0 (3.52" 240x360 Tri-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor Hardware**: Waveshare 3.52inch e-Paper HAT (B)
//! - **Controller IC**: UC8253, driven through
//!   [`Uc8253Variant::Se0352n14`](crate::controllers::Uc8253Variant::Se0352n14). The default
//!   [`Uc8253Variant::Gdey037t03`](crate::controllers::Uc8253Variant::Gdey037t03) profile will
//!   *not* drive this panel: the init sequence, RAM plane order and refresh all differ.
//! - **Native Resolution**: 240 x 360 pixels (already byte-aligned, no RAM padding).
//!   Waveshare advertise the panel as 360 x 240 — that is the landscape viewing orientation, not
//!   the raster. Controller RAM is 30 bytes per line over 360 lines; transposing these keeps the
//!   buffer the right total size but strides it 45 bytes per line, shearing and repeating the
//!   image.
//! - **Identification**: the FPC ribbon is stamped `SE0352N01FPC-A 2024.12.04 X`. Note the flex
//!   carries the **`N01`** part number, not this panel's `N14` — the black-and-white and
//!   tri-color 3.52" panels share one flex, so the ribbon identifies the family, not the variant.
//!   The date is the flex production run; the trailing `X` is unexplained.
//! - **Orientation**: `(0,0)` is top-left with the FPC ribbon at the **top** (verified on
//!   hardware). Mounted ribbon-down the image reads rotated 180°, which
//!   [`DisplayRotation::Rotate180`](crate::graphics::buffer::DisplayRotation) compensates for.
//! - **Busy Polarity**: Active-**LOW** (busy while LOW), as for the other UC8253 panel here.
//! - **Refresh**: full refresh only, roughly 16–20 s.
//! - **Duty cycle**: Waveshare specify **at least 180 s between refreshes**, and **at least one
//!   refresh every 24 h**. Driving it faster degrades the image — a run of full refreshes ~19 s
//!   apart rendered cleanly at first, then decayed into streaked bands that persisted into later
//!   runs. Leaving it un-refreshed for days risks lasting image retention instead. There is no partial or fast waveform — the
//!   red pigment is a heavier particle needing the full OTP waveform to migrate.
//! - **Ink Polarity**: set bits are ink and `0x00` is white in **both** RAM planes, so both
//!   channels clear with `clear_frame(channel, 0x00)`. This is the opposite of the monochrome
//!   [`GDEY037T03`](super::GDEY037T03), which clears to `0xFF`, and follows from the `CDI` value
//!   (`0x87`) the controller writes during init.
//! - **White Point**: a cleared panel reads grey next to a monochrome one. That is this panel's
//!   white point, not a fault.
//!
//! ### Vendor References
//! - Product page: <https://www.waveshare.com/3.52inch-e-paper-hat-b.htm>
//! - Wiki: <https://www.waveshare.com/wiki/3.52inch_e-Paper_HAT_(B)>
//! - Manual: <https://www.waveshare.com/wiki/3.52inch_e-Paper_HAT_(B)_Manual>
//! - User manual PDF: <https://files.waveshare.com/wiki/3.52inch%20e-Paper%20HAT%20(B)/3.52inch-e-Paper_(B)-user-manual.pdf>
//! - Reference driver: <https://github.com/waveshareteam/e-Paper/tree/master/E-paper_Separate_Program/3in52_e-Paper_B>

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for the Waveshare 3.52" e-Paper HAT (B) (UC8253).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SE0352N14TNGA0;

impl EpdPanel for SE0352N14TNGA0 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 240;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 360;

    /// Panel color operating mode (Tri-Color: Black, White, Red).
    const COLOR_MODE: ColorMode = ColorMode::TriColor;
}

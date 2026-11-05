//! Panel specification for GDEY042T81 (4.2" 400 x 300 Monochrome e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor Hardware**: Good Display `GDEY042T81`, 24-pin FPC. Not the `GDEY042T81-FL02`
//!   front-light variant, which has a different specification sheet. The rear sticker on the
//!   unit used for bring-up reads `042BN-T81-D2` and the FPC carries no print. GxEPD2 and
//!   `buy-lcd.com` also know this glass as `GDEY042T91`, an early pre-production name.
//! - **Controller IC**: SSD1683, driven through
//!   [`Ssd168xVariant::Ssd1683`](crate::controllers::Ssd168xVariant::Ssd1683). The variant is
//!   **not** inferred from the panel type (`EpdController` never sees `PANEL`), so build the
//!   controller with `Ssd168xController::for_panel::<GDEY042T81>(Ssd168xVariant::Ssd1683)`.
//!   Pairing the panel with another variant does not error: it sends the wrong `0x21` and
//!   refresh bytes and renders badly.
//! - **Native Resolution**: 400 x 300 pixels (spec: 400(H) x 300(V), 84.8 x 63.6 mm active
//!   area, 120 dpi).
//! - **RAM Alignment**: none needed. 400 is a multiple of 8, so a row is 50 bytes and a frame
//!   15,000 bytes. X window end byte is `0x31`, Y end is `0x012B`.
//! - **Busy Polarity**: Active-**HIGH** (busy while HIGH), per spec note 5-4 and the vendor
//!   demo's `Epaper_READBUSY`.
//! - **Reference sources**: Good Display `AU-GDEY042T81-FP4G-20240801` demo (init, `0xF7` full,
//!   `0xC7` fast, `0xFF` partial, deep sleep with a 100 ms settle), the plain `GDEY042T81`
//!   specification sheet and the SSD1683 datasheet Rev 1.0. GxEPD2 `GxEPD2_420_GDEY042T81` drives
//!   the same glass and uses `0xFC` for partial where the vendor sends `0xFF`.
//!
//! ### Deliberate departures from the vendor demo
//! - Data entry mode is `0x03` (Y increments), the crate-wide SSD168x convention. The demo uses
//!   `0x01` and starts Y at 299. Same image, opposite sweep.
//! - `0x18 = 80` (internal temperature sensor) is written at init, as GxEPD2 and ESPHome do. The
//!   demo omits it.
//! - The partial refresh does not hard-reset first. See [`Ssd168xVariant::Ssd1683`](crate::controllers::Ssd168xVariant::Ssd1683).
//!
//! ### Gray4
//! The panel also does 4-level grayscale (the spec lists a 2 s "4 Gray update"). Use
//! [`Ssd168xRefreshMode::Gray4Otp`](crate::controllers::Ssd168xRefreshMode::Gray4Otp), which runs
//! the built-in waveform with no LUT upload, so the panel has no `GRAY4` register bundle. Draw
//! with [`Gray4Polarity::ADAFRUIT_SSD1677`](crate::graphics::buffer::Gray4Polarity::ADAFRUIT_SSD1677)
//! (both planes inverted), which matches how the vendor splits its 2 bits-per-pixel image.
//!
//! ### Status
//! Register sequences derived from the vendor demo and datasheet above and pinned by byte-exact
//! tests. Not yet verified on physical hardware, `Gray4Otp` included.
//!
//! ### Vendor References
//! - Good Display product page: <https://www.good-display.com/product/386.html>
//! - Specification: <https://v4.cecdn.yun300.cn/100001_1909185148/GDEY042T81.pdf>
//! - SSD1683 datasheet: <https://v4.cecdn.yun300.cn/100001_1909185148/SSD1683.pdf>
//! - Demo: <https://v4.cecdn.yun300.cn/100001_1909185148/AU-GDEY042T81.rar>
//! - GxEPD2 reference: <https://github.com/ZinggJM/GxEPD2/blob/master/src/gdey/GxEPD2_420_GDEY042T81.cpp>

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel specification for the Good Display GDEY042T81 (SSD1683, 400 x 300).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEY042T81;

impl EpdPanel for GDEY042T81 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 400;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 300;

    /// Panel color operating mode (Monochrome Black and White).
    const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
}

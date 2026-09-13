//! Panel specification for GDEM0154F51H (1.54" 200x200 Quad-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor Hardware**: Good Display `GDEM0154F51H`, sold by Waveshare as the *1.54inch
//!   e-Paper (G)* module (SKU 30441, FPC-8101).
//! - **Controller IC**: JD79660AA, driven through
//!   [`Jd79660Controller`](crate::controllers::Jd79660Controller) — a thin wrapper over the
//!   shared [`Jd7966xController`](crate::controllers::Jd7966xController), the same relationship
//!   [`Jd79661Controller`](crate::controllers::Jd79661Controller) has to it for
//!   [`ZJY122250_0213AJH_E5`](crate::panels::ZJY122250_0213AJH_E5).
//! - **Native Resolution**: 200 x 200 pixels. Already RAM-aligned (multiple of 8) — no
//!   `RAM_WIDTH` override needed, unlike `ZJY122250_0213AJH_E5`'s 122->128px padding.
//! - **Pixel format**: 2 bits/pixel, MSB-first, 4 px/byte. Color codes confirmed from Waveshare's
//!   `EPD_1in54g.h` and cross-checked against GxEPD2's `writeScreenBuffer`'s all-white clear byte
//!   `0x55`: Black=`0b00`, White=`0b01`, Yellow=`0b10`, Red=`0b11`.
//! - **Busy Polarity**: Active-LOW (busy while LOW), same convention as `Jd79661Controller`.
//! - **Refresh**: full refresh only (~20s per vendor spec); no partial-window support in this
//!   port, even though the silicon has one (`R83H`/PTL per the JD79660A datasheet) — matches the
//!   vendor's own reference driver, which never uses it either.
//! - **Status**: byte-for-byte agreement between Waveshare's `EPD_1in54g.c`/`.h` and GxEPD2's
//!   `GxEPD2_154c_GDEM0154F51H.cpp`/`.h`, cross-checked against the JD79660A datasheet (v1.0.3)
//!   directly for register lengths/defaults (confirms `POF`/`DRF` both take a `0x00` data byte;
//!   `0x4D`/`0xE7`/`0xE9`/`0xB4`/`0xB5` are undocumented in the public datasheet). No
//!   `Adafruit_EPD` board exists for this panel/IC (checked — no coverage). **Not yet confirmed
//!   on physical hardware.**
//!
//! ### Vendor References
//! - Waveshare product page: <https://www.waveshare.com/1.54inch-e-paper-g.htm>
//! - Waveshare wiki: <https://www.waveshare.com/wiki/1.54inch_e-Paper_Module_(G)>
//! - Waveshare manual: <https://www.waveshare.com/wiki/1.54inch_e-Paper_Module_(G)_Manual>
//! - Datasheet (JD79660AA v1.0.3): linked from GxEPD2's header (cecdn.yun300.cn host)
//! - Waveshare reference driver: <https://github.com/waveshareteam/e-Paper/blob/master/E-paper_Separate_Program/1in54_e-Paper_G/RaspberryPi_JetsonNano/c/lib/e-Paper/EPD_1in54g.c>
//! - GxEPD2 reference driver: <https://github.com/ZinggJM/GxEPD2/blob/master/src/epd4c/GxEPD2_154c_GDEM0154F51H.h>

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for Good Display GDEM0154F51H / Waveshare 1.54inch
/// e-Paper (G) (SKU 30441).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEM0154F51H;

/// GxEPD2 reference alias for this panel (`GxEPD2_154c_GDEM0154F51H`).
#[allow(non_camel_case_types)]
pub type GxEPD2_154c_GDEM0154F51H = GDEM0154F51H;

impl EpdPanel for GDEM0154F51H {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 200;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 200;

    /// Panel color operating mode (Quad-Color: Black, White, Red, Yellow).
    const COLOR_MODE: ColorMode = ColorMode::QuadColor;

    // RAM_WIDTH left at default (200) — already byte-aligned, no override needed.
}

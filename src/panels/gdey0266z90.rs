//! Panel specification for GDEY0266Z90 (2.66" 152x296 Tri-Color e-Paper display).
//!
//! ### Hardware Notes:
//! - **Vendor Hardware**: Good Display `GDEY0266Z90`, sold by Waveshare as the
//!   *2.66inch e-Paper Module (B)*. GxEPD2 treats the two as one panel
//!   (`Waveshare_2_66_bwr = GDEY0266Z90`).
//! - **Controller IC**: SSD1680 (`SSD1680Z8` on the Waveshare listing), driven through the default
//!   [`Ssd168xVariant::Ssd1680`](crate::controllers::Ssd168xVariant::Ssd1680) profile — the same
//!   one the monochrome `GDEM0213B74` uses. No variant selection is needed for this panel.
//! - **Native Resolution**: 152 x 296 pixels. Both vendors advertise the panel as 296 x 152 — that
//!   is the landscape viewing orientation, not the raster. `GxEPD2_266c` likewise declares
//!   `WIDTH = 152, HEIGHT = 296`, as does [`E2266KS0C1`](crate::panels::E2266KS0C1), which is the
//!   same 2.66" glass behind a Pervasive Displays COG. Transposing the two shears the image.
//! - **RAM Alignment**: none needed. 152 is a multiple of 8, so a row is exactly 19 bytes and a
//!   plane 5624 bytes. The SSD1680's RAM is 176 x 296, so the panel fits without padding.
//! - **Identification**: the glass this driver was written against is stamped
//!   `DEPG0266RWS800F34HP` with a second line `N2405P10213-01-32043-1`, and the FPC ribbon reads
//!   `FPC-7510 Rev. C`. That decodes almost completely against DKE's 2.66" family:
//!
//!   | Field | Meaning |
//!   | :--- | :--- |
//!   | `DEPG` | **DKE Group**'s prefix for active-matrix graphic EPDs (their segment parts are `DEP0025` / `DEP0055`) |
//!   | `0266` | 2.66" diagonal, 152 x 296 across the whole family |
//!   | `RW` | Red/White — three-colour. Siblings are `BN` (B/W), `BS` (B/W, freezer grade) and `YN` (B/W/Y) |
//!   | `S800` | **the driver IC: SSD1680.** The same family ships as `…F51B…` (JD79651B) and `…U25D…` (UC8251d), which are *not* interchangeable with this driver |
//!   | `F34` | FPC tail variant; `F1`, `F23` and `F36` exist on the same glass |
//!   | `HP` | undocumented suffix, most likely a grade or process marker |
//!
//!   `N2405P10213-01-32043-1` is a production traceability string, not a part number: `N2405` is
//!   very likely a 2024 date code, the rest a lot and panel serial. `FPC-7510` is the tail's own
//!   designator, and GxEPD2 uses exactly that marking to identify this family — its display
//!   selection headers tag `GxEPD2_266_BN` as "DEPG0266BN 152x296, SSD1680, (FPC7510)". `Rev. C`
//!   is the flex artwork revision.
//!
//!   So this particular unit is **DKE glass, not Good Display's**, despite the `GDEY0266Z90` name.
//!   That is expected — Waveshare source this module from more than one supplier behind the same
//!   152 x 296 BWR SSD1680 interface — and it is cross-validated rather than assumed: GxEPD2's
//!   `src/epd/GxEPD2_266_BN.cpp`, written for DKE's monochrome `DEPG0266BN`, has an `_InitDisplay()`
//!   whose register set and values are **identical** to the `GxEPD2_266c` sequence this driver
//!   follows, differing only in the order of the mutually independent `0x21` and `0x18` writes.
//!
//!   What a different glass vendor *can* still change is the **OTP waveform**: an image that comes
//!   out geometrically correct with odd ghosting or a weak red is a waveform difference, not a
//!   register fault, and nothing in `epdsi` selects it. Note that the monochrome DKE sibling
//!   advertises a 4 s full and 800 ms partial refresh against this panel's ~18–20 s, which is the
//!   red pigment's cost, not a driver difference.
//! - **Orientation**: `(0,0)` is top-left in the vendor's own orientation. Seated in an Adafruit
//!   Feather ThinkInk 24-pin FPC connector the raster lands 180° round, which
//!   [`DisplayRotation::Rotate180`](crate::graphics::buffer::DisplayRotation) compensates for.
//!   That offset is a property of the connector, not of the panel.
//! - **Busy Polarity**: Active-**HIGH** (busy while HIGH), matching the SSD168x controller's
//!   hard-coded polling polarity. Do not invert it to match the UC8253 panels.
//! - **Refresh**: full refresh only. `GxEPD2_266c` reports `hasPartialUpdate = true` but "refresh
//!   is full screen", with `partial_refresh_time == full_refresh_time == 18000` ms and
//!   `hasFastPartialUpdate = false`. Measured on hardware (RP2350, DKE glass):
//!
//!   | Mode | Measured |
//!   | :--- | ---: |
//!   | [`Full`](crate::controllers::Ssd168xRefreshMode::Full) | 20.0 s |
//!   | `Full`, windowed via `set_window` | 20.0 s — a narrower window costs the same |
//!   | [`FastFull`](crate::controllers::Ssd168xRefreshMode::FastFull) | 16.2 s |
//!   | [`BaseMap`](crate::controllers::Ssd168xRefreshMode::BaseMap) | 19.9 s |
//!   | [`Partial`](crate::controllers::Ssd168xRefreshMode::Partial) | 19.9 s |
//!
//!   `FastFull` is a real 19 % saving on this glass, though Good Display quote only ~19 s against
//!   ~20 s on their own — the OTP waveform differs by supplier, so measure rather than assume.
//!   `Partial` is neither fast nor differential here and additionally drops red content, so it has
//!   no use on this panel; `BaseMap` likewise buys nothing, since there is no differential mode for
//!   it to prime. Both exist for parity with Good Display's reference driver.
//! - **RAM plane roles**: `0x26` is *always* the Red plane on this panel, unlike a monochrome
//!   SSD1680 panel where it doubles as the previous-frame buffer for differential updates. Seeding
//!   it with a Black/White image — the idiom that is correct on the `GDEM0213B74` — sets nearly
//!   every bit and renders the region solid red.
//! - **Duty cycle**: Waveshare specify **at least 180 s between refreshes**, and at least one
//!   update every 24 h to avoid burn-in. Neither is enforced here — `refresh` will happily run
//!   back to back — so any long-running firmware must pace itself and `sleep()` in between.
//! - **Ink Polarity**: the two RAM planes **disagree**, and getting this wrong mis-renders silently.
//!   In the Black/White plane (`0x24`) `0xFF` is white and a cleared bit is black, the usual
//!   monochrome convention. The Red plane (`0x26`) is **inverted**: `0x00` is no red and a *set*
//!   bit is red. So a white panel is `clear_frame(ColorChannel::BlackWhite, 0xFF)` plus
//!   `clear_frame(ColorChannel::RedYellow, 0x00)` — the same asymmetry as the `GDEM0154Z90`, and
//!   the opposite of the `SE0352N14TNGA0`, which clears both planes to `0x00`. All three reference
//!   drivers agree: GxEPD2 writes `~color`, as do Waveshare's `epd2in66b.cpp` and Good Display's
//!   `EPD_WhiteScreen_ALL()`.
//!
//!   [`PageBuffer::set_pixel`](crate::graphics::buffer::PageBuffer::set_pixel) *clears* the bit for
//!   `black: true`, so it matches the Black/White plane directly; render the red plane with the
//!   polarity flipped (clear byte `0x00`, red drawn as `black: false`).
//! - **Status**: verified on hardware — RP2350 Pico 2 over a Good Display DESPI-C02, driving all
//!   four refresh modes. Both planes render with correct polarity and orientation, and the timings
//!   above are measured rather than quoted.
//!
//! ### Vendor References
//! - Good Display product page: <https://www.good-display.com/product/430.html>
//! - Waveshare product page: <https://www.waveshare.com/2.66inch-e-Paper-B.htm>
//! - Waveshare wiki: <http://www.waveshare.com/wiki/2.66inch_e-Paper_Module_(B)>
//! - Waveshare manual: <http://www.waveshare.com/wiki/2.66inch_e-Paper_Module_(B)_Manual>
//! - Datasheet: <https://files.waveshare.com/upload/e/ec/2.66inch-e-paper-b-specification.pdf>
//! - Waveshare reference driver: <https://github.com/waveshareteam/e-Paper/blob/master/RaspberryPi_JetsonNano/c/lib/e-Paper/EPD_2in66b.c>
//! - GxEPD2 reference driver: <https://github.com/ZinggJM/GxEPD2/blob/master/src/epd3c/GxEPD2_266c.h>

use crate::traits::{ColorMode, EpdPanel};

/// Physical panel driver specification for the Good Display GDEY0266Z90 / Waveshare 2.66" e-Paper
/// Module (B) (SSD1680).
#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GDEY0266Z90;

/// GxEPD2 reference alias for this panel (`GxEPD2_266c`).
#[allow(non_camel_case_types)]
pub type GxEPD2_266c = GDEY0266Z90;

impl EpdPanel for GDEY0266Z90 {
    /// Panel physical width in pixels.
    const WIDTH: u32 = 152;

    /// Panel physical height in pixels.
    const HEIGHT: u32 = 296;

    /// Panel color operating mode (Tri-Color: Black, White, Red).
    const COLOR_MODE: ColorMode = ColorMode::TriColor;
}

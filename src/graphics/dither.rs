//! Zero-heap Bayer ordered dithering from RGB to the panel palettes `epdsi` can drive.
//!
//! Every function here is stateless: it takes the pixel's position and its source color and
//! returns the palette color to draw. Ordered dithering needs nothing else, so there is no line
//! buffer and no allocation, which is what makes it usable inside a
//! [`render_paged`](crate::graphics::render_paged) closure on a microcontroller.
//!
//! Pass the same `(x, y)` you draw at. Inside a paged closure the drawing coordinates are
//! already absolute panel coordinates, so the pattern stays continuous across page boundaries
//! without any extra offset.
//!
//! The threshold matrix is a 4x4 Bayer matrix (16 levels). Error diffusion is not provided: it
//! needs per-line error buffers, which is a separate piece of work.
//!
//! ```
//! use embedded_graphics_core::pixelcolor::Rgb888;
//! use epdsi::graphics::dither::dither_binary;
//!
//! // Mid gray comes out as a mix of black and white, not a flat color.
//! let on = (0..4)
//!     .flat_map(|y| (0..4).map(move |x| (x, y)))
//!     .filter(|&(x, y)| dither_binary(x, y, Rgb888::new(128, 128, 128)).is_on())
//!     .count();
//! assert_eq!(on, 8);
//! ```

use embedded_graphics_core::pixelcolor::{BinaryColor, Rgb888, RgbColor};

use crate::graphics::buffer::{Gray4Color, TriColor};
use crate::traits::SevenColor;

/// 4x4 Bayer threshold matrix, values `0..=15`.
const BAYER4: [[u8; 4]; 4] = [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Threshold for `(x, y)` scaled to `0..=255`, centered so the average over the matrix is 127.5.
fn threshold(x: u32, y: u32) -> i32 {
    let m = BAYER4[(y % 4) as usize][(x % 4) as usize] as i32;
    (2 * m + 1) * 255 / 32
}

/// Rec. 601 luma, `0..=255`.
fn luma(c: Rgb888) -> i32 {
    (c.r() as i32 * 77 + c.g() as i32 * 150 + c.b() as i32 * 29) >> 8
}

/// Quantizes a `0..=255` value to `0..levels`, using the Bayer threshold to pick between the two
/// nearest levels.
fn quantize(x: u32, y: u32, value: i32, levels: i32) -> i32 {
    let scaled = value.clamp(0, 255) * (levels - 1);
    let level = scaled / 255;
    let rem = scaled % 255;
    if rem > threshold(x, y) {
        (level + 1).min(levels - 1)
    } else {
        level
    }
}

/// Picks the nearest palette entry after biasing each channel by the Bayer threshold.
///
/// The bias spans roughly -123..=+120 per channel, which is right for palettes whose channels
/// are all `0` or `255` (the ideal primaries used below). Returns an index into `palette`.
fn nearest_biased(x: u32, y: u32, c: Rgb888, palette: &[Rgb888]) -> usize {
    let bias = threshold(x, y) - 127;
    let r = c.r() as i32 + bias;
    let g = c.g() as i32 + bias;
    let b = c.b() as i32 + bias;
    let mut best = 0;
    let mut best_dist = i32::MAX;
    for (i, p) in palette.iter().enumerate() {
        let dr = r - p.r() as i32;
        let dg = g - p.g() as i32;
        let db = b - p.b() as i32;
        let dist = dr * dr + dg * dg + db * db;
        if dist < best_dist {
            best_dist = dist;
            best = i;
        }
    }
    best
}

/// Dithers to black and white. `BinaryColor::On` is black ink, matching
/// [`PageBuffer`](crate::graphics::PageBuffer).
pub fn dither_binary(x: u32, y: u32, color: Rgb888) -> BinaryColor {
    if quantize(x, y, luma(color), 2) == 0 {
        BinaryColor::On
    } else {
        BinaryColor::Off
    }
}

/// Dithers to the four gray levels of a Gray4 panel.
pub fn dither_gray4(x: u32, y: u32, color: Rgb888) -> Gray4Color {
    match quantize(x, y, luma(color), 4) {
        0 => Gray4Color::Black,
        1 => Gray4Color::Dark,
        2 => Gray4Color::Light,
        _ => Gray4Color::White,
    }
}

/// Dithers to Black, White and one accent ink.
///
/// `accent` is the RGB value to treat as the panel's accent ink: pass `Rgb888::RED` for a
/// black/white/red panel or `Rgb888::YELLOW` for black/white/yellow. Use fully saturated values;
/// the bias assumes palette channels are `0` or `255`.
pub fn dither_tri(x: u32, y: u32, color: Rgb888, accent: Rgb888) -> TriColor {
    let palette = [Rgb888::BLACK, Rgb888::WHITE, accent];
    match nearest_biased(x, y, color, &palette) {
        0 => TriColor::Black,
        1 => TriColor::White,
        _ => TriColor::Accent,
    }
}

/// Dithers to the six Spectra 6 colors: Black, White, Yellow, Red, Blue and Green.
///
/// `SevenColor::Orange` and `SevenColor::Clean` are never returned. The palette uses ideal
/// primaries, not measured ink colors, and real panels render their inks more muted than that.
///
/// On a `GDEP073E01` this keeps tonal range and fine detail that nearest-color snapping loses,
/// so photos look better dithered. The result is duller and browner than the source. A palette
/// calibrated to measured ink colors is not provided.
pub fn dither_seven(x: u32, y: u32, color: Rgb888) -> SevenColor {
    const PALETTE: [Rgb888; 6] = [
        Rgb888::BLACK,
        Rgb888::WHITE,
        Rgb888::YELLOW,
        Rgb888::RED,
        Rgb888::BLUE,
        Rgb888::GREEN,
    ];
    const CODES: [SevenColor; 6] = [
        SevenColor::Black,
        SevenColor::White,
        SevenColor::Yellow,
        SevenColor::Red,
        SevenColor::Blue,
        SevenColor::Green,
    ];
    CODES[nearest_biased(x, y, color, &PALETTE)]
}

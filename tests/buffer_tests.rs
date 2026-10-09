#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only.

//! Regression tests for [`PageBuffer`] row addressing.
//!
//! Panels whose visible width is not a multiple of 8 (the 122 px GDEM0213B74 and
//! ZJY122250_0213AJH_E5) are addressed in whole bytes by their controllers, so a row occupies
//! `width.div_ceil(8)` bytes. Computing the row offset as `y * width / 8` instead shears the
//! image by one bit per row.

use embedded_graphics_core::pixelcolor::BinaryColor;
use embedded_graphics_core::prelude::*;
use epdsi::prelude::*;

/// A 122 px row must occupy 16 bytes, not 15.
#[test]
fn stride_rounds_up_for_non_byte_aligned_width() {
    let mut data = [0xFFu8; 16 * 4];
    let buffer = PageBuffer::new(&mut data, 122, 4, 0);

    assert_eq!(buffer.stride(), 16);
    assert_eq!(buffer.width(), 122, "visible width must stay unpadded");
}

/// A byte-aligned width is unaffected, so every existing panel keeps its current layout.
#[test]
fn stride_is_unchanged_for_byte_aligned_width() {
    let mut data = [0xFFu8; 25 * 4];
    let buffer = PageBuffer::new(&mut data, 200, 4, 0);

    assert_eq!(buffer.stride(), 25);
}

/// The first pixel of each row must land on a byte boundary of that row.
#[test]
fn rows_start_on_byte_boundaries_when_width_is_not_aligned() {
    let mut data = [0xFFu8; 16 * 4];
    {
        let mut buffer = PageBuffer::new(&mut data, 122, 4, 0);
        for y in 0..4 {
            buffer.set_pixel(0, y, true);
        }
    }

    // Bit 7 of the first byte of each 16-byte row is cleared; nothing else changed.
    for (row, chunk) in data.chunks(16).enumerate() {
        assert_eq!(
            chunk[0], 0x7F,
            "row {row} pixel 0 landed at the wrong offset"
        );
        assert!(chunk[1..].iter().all(|&b| b == 0xFF), "row {row} smeared");
    }
}

/// Pixels in the off-panel padding (x = 122..127) must be discarded, not wrapped into the
/// next row.
#[test]
fn pixels_beyond_visible_width_are_clipped() {
    let mut data = [0xFFu8; 16 * 4];
    {
        let mut buffer = PageBuffer::new(&mut data, 122, 4, 0);
        buffer.set_pixel(125, 0, true);
    }

    assert!(
        data.iter().all(|&b| b == 0xFF),
        "a pixel outside the visible width was written"
    );
}

/// `y_offset` addresses a sub-region while embedded-graphics coordinates stay in panel space.
#[test]
fn y_offset_maps_band_coordinates_with_padded_stride() {
    let mut data = [0xFFu8; 16 * 4];
    {
        let mut buffer = PageBuffer::new(&mut data, 122, 4, 100);

        // Panel row 102 is local row 2 of this band.
        Pixel(Point::new(0, 102), BinaryColor::On)
            .draw(&mut buffer)
            .unwrap();
    }

    assert_eq!(data[2 * 16], 0x7F);
    assert_eq!(data[0], 0xFF);
}

/// Rotate180 on a full-frame buffer must map (0,0) to the very last pixel of the last row.
#[test]
fn rotate180_maps_origin_to_final_pixel() {
    const W: u32 = 240;
    const H: u32 = 416;
    const STRIDE: usize = 30;
    let mut data = [0xFFu8; STRIDE * H as usize];
    {
        let mut buffer = PageBuffer::new(&mut data, W, H, 0);
        buffer.set_rotation(DisplayRotation::Rotate180);
        buffer.set_pixel(0, 0, true);
    }
    let last = STRIDE * H as usize - 1;
    assert_eq!(data[last], 0xFE, "(0,0) did not land at the last pixel");
    assert!(
        data[..last].iter().all(|&b| b == 0xFF),
        "something else was written"
    );
}

/// `Rotate180` must produce byte-identical output to an explicit 180-degree blit of the same
/// pattern. The transform probe used the explicit blit and rendered correctly on hardware, while
/// the same content drawn through `set_rotation(Rotate180)` did not.
#[test]
fn rotate180_matches_explicit_blit() {
    use embedded_graphics_core::primitives::Rectangle;

    const W: u32 = 240;
    const H: u32 = 416;
    const STRIDE: usize = 30;
    const N: usize = STRIDE * H as usize;

    fn pattern(buf: &mut PageBuffer) {
        // Asymmetric in both axes: a block near the origin plus a short run along the top edge.
        for y in 4..20u32 {
            for x in 4..20u32 {
                buf.set_pixel(x, y, true);
            }
        }
        for x in 0..120u32 {
            buf.set_pixel(x, 0, true);
        }
        for y in 0..200u32 {
            buf.set_pixel(0, y, true);
        }
    }

    // A: drawn through PageBuffer's own Rotate180
    let mut a = [0xFFu8; N];
    {
        let mut buf = PageBuffer::new(&mut a, W, H, 0);
        buf.set_rotation(DisplayRotation::Rotate180);
        pattern(&mut buf);
    }

    // B: drawn unrotated, then blitted through explicit 180-degree coordinate math
    let mut tmp = [0xFFu8; N];
    {
        let mut buf = PageBuffer::new(&mut tmp, W, H, 0);
        pattern(&mut buf);
    }
    let mut b = [0xFFu8; N];
    {
        let mut buf = PageBuffer::new(&mut b, W, H, 0);
        for y in 0..H {
            for x in 0..W {
                let idx = y as usize * STRIDE + (x / 8) as usize;
                let bit = 7 - (x % 8);
                if tmp[idx] & (1 << bit) == 0 {
                    buf.set_pixel(W - 1 - x, H - 1 - y, true);
                }
            }
        }
    }

    let _ = Rectangle::new(
        embedded_graphics_core::geometry::Point::zero(),
        embedded_graphics_core::geometry::Size::new(W, H),
    );

    let first_diff = a.iter().zip(b.iter()).position(|(x, y)| x != y);
    assert_eq!(
        first_diff, None,
        "Rotate180 diverges from an explicit blit at byte {first_diff:?}"
    );
}

/// Regression tests for [`PageBufferPair`] / [`TriColor`] / [`PlanePolarity`] — the paired
/// Black/White + accent plane draw target Tri-Color panels use.
///
/// `PlanePolarity::SSD168X` (Black/White plane normal, accent plane inverted — a *set* bit is
/// red/yellow) is the convention hardware-verified against `GDEY0266Z90`/`GDEM0154Z90`; the
/// buffers below are seeded with each plane's own background byte under that polarity
/// (`0xFF` bw, `0x00` accent), not one shared fill value, because the two planes disagree.
mod page_buffer_pair {
    use super::*;

    /// Each `TriColor` variant must write only the plane it owns, leaving the other at its
    /// background fill.
    #[test]
    fn each_color_writes_only_its_own_plane() {
        let mut bw = [0xFFu8; 4];
        let mut accent = [0x00u8; 4];
        {
            let mut pair =
                PageBufferPair::new(&mut bw, &mut accent, 32, 1, 0, PlanePolarity::SSD168X);
            pair.set_pixel(0, 0, TriColor::Black);
            pair.set_pixel(1, 0, TriColor::Accent);
            pair.set_pixel(2, 0, TriColor::White);
        }

        // Black: bw bit cleared (ink), accent untouched (still background, 0).
        assert_eq!(bw[0] & 0x80, 0x00);
        assert_eq!(accent[0] & 0x80, 0x00);
        // Accent: accent bit set (ink, inverted plane), bw untouched (still background, 1).
        assert_eq!(bw[0] & 0x40, 0x40);
        assert_eq!(accent[0] & 0x40, 0x40);
        // White: both planes at their own background.
        assert_eq!(bw[0] & 0x20, 0x20);
        assert_eq!(accent[0] & 0x20, 0x00);
    }

    /// Redrawing a pixel with a different color must not leave a stale bit on the plane the
    /// previous color used — otherwise an accent pixel painted over with black would still
    /// render red/yellow underneath the black ink.
    #[test]
    fn redrawing_a_pixel_clears_the_previous_colors_plane() {
        let mut bw = [0xFFu8; 4];
        let mut accent = [0x00u8; 4];
        {
            let mut pair =
                PageBufferPair::new(&mut bw, &mut accent, 32, 1, 0, PlanePolarity::SSD168X);
            pair.set_pixel(0, 0, TriColor::Accent);
            pair.set_pixel(0, 0, TriColor::Black);
        }

        assert_eq!(
            bw[0] & 0x80,
            0x00,
            "black bit should be cleared on bw plane"
        );
        assert_eq!(
            accent[0] & 0x80,
            0x00,
            "accent plane must be cleared back to background, not left set from the earlier draw"
        );
    }

    /// `PageBufferPair` must work as an `embedded-graphics` `DrawTarget<Color = TriColor>`, not
    /// just through its own `set_pixel`.
    #[test]
    fn draw_target_routes_pixels_to_the_correct_plane() {
        let mut bw = [0xFFu8; 4];
        let mut accent = [0x00u8; 4];
        {
            let mut pair =
                PageBufferPair::new(&mut bw, &mut accent, 32, 1, 0, PlanePolarity::SSD168X);
            Pixel(Point::new(1, 0), TriColor::Accent)
                .draw(&mut pair)
                .unwrap();
        }

        assert_eq!(
            accent[0] & 0x40,
            0x40,
            "accent ink is the set bit on this polarity"
        );
        assert_eq!(bw[0] & 0x40, 0x40, "bw plane stays at background");
    }

    /// Swapping to [`PlanePolarity::UC8253`] (both planes inverted, unlike SSD168X where only
    /// the accent plane is) must flip the Black/White plane's ink bit too — this is the exact
    /// bug that shipped in the first cut of `PageBufferPair`, where the mapping was hardcoded to
    /// the SSD168X convention regardless of the `polarity` a caller might supply.
    #[test]
    fn uc8253_polarity_inverts_the_bw_plane_ink_bit_too() {
        let mut bw = [0x00u8; 4];
        let mut accent = [0x00u8; 4];
        {
            let mut pair =
                PageBufferPair::new(&mut bw, &mut accent, 32, 1, 0, PlanePolarity::UC8253);
            pair.set_pixel(0, 0, TriColor::Black);
            pair.set_pixel(1, 0, TriColor::Accent);
        }

        // Under UC8253 polarity ink is the *set* bit on both planes, so Black now sets the bw
        // bit (opposite of SSD168X, where the same call clears it).
        assert_eq!(bw[0] & 0x80, 0x80, "UC8253 bw ink is the set bit");
        assert_eq!(
            accent[0] & 0x80,
            0x00,
            "bw draw must leave accent at background"
        );
        assert_eq!(
            bw[0] & 0x40,
            0x00,
            "accent draw must leave bw at background"
        );
        assert_eq!(accent[0] & 0x40, 0x40, "UC8253 accent ink is the set bit");
    }

    /// `clear` must reset both planes to their own background byte under this pair's
    /// `polarity`, undoing whatever a previous draw left behind on both planes at once.
    #[test]
    fn clear_resets_both_planes_to_their_own_background() {
        let mut bw = [0x00u8; 4];
        let mut accent = [0xFFu8; 4];
        {
            let mut pair =
                PageBufferPair::new(&mut bw, &mut accent, 32, 1, 0, PlanePolarity::SSD168X);
            pair.set_pixel(0, 0, TriColor::Black);
            pair.set_pixel(1, 0, TriColor::Accent);
            pair.clear();
        }

        assert_eq!(
            bw, [0xFFu8; 4],
            "bw plane must be back at its own background (0xFF)"
        );
        assert_eq!(
            accent, [0x00u8; 4],
            "accent plane must be back at its own background (0x00)"
        );
    }

    /// `bounding_box` must reflect the shared width/height/`y_offset`, the same as a plain
    /// `PageBuffer`.
    #[test]
    fn bounding_box_matches_the_shared_page_geometry() {
        let mut bw = [0xFFu8; 4];
        let mut accent = [0x00u8; 4];
        let pair = PageBufferPair::new(&mut bw, &mut accent, 32, 4, 8, PlanePolarity::SSD168X);

        let bb = pair.bounding_box();
        assert_eq!(bb.top_left, Point::new(0, 8));
        assert_eq!(bb.size, Size::new(32, 4));
    }
}

/// Regression tests for [`GrayBufferPair`] / [`Gray4Color`] / [`Gray4Polarity`] — the paired
/// Black/White + Red/Yellow plane draw target Adafruit_EPD-style Gray4 mode reuses.
///
/// `Gray4Polarity::ADAFRUIT_SSD1680` (neither plane inverted — a *set* bit is the code bit
/// directly) is the only convention `epdsi` has evidence for so far (`GDEY0266T90`).
mod gray_buffer_pair {
    use super::*;

    /// Each `Gray4Color` variant must write the exact (plane_a, plane_b) code bits Adafruit_EPD's
    /// `layer_colors` uses: `White=00, Light=01, Dark=10, Black=11`.
    #[test]
    fn each_color_writes_its_own_two_bit_code() {
        let mut plane_a = [0x00u8; 4];
        let mut plane_b = [0x00u8; 4];
        {
            let mut pair = GrayBufferPair::new(
                &mut plane_a,
                &mut plane_b,
                32,
                1,
                0,
                Gray4Polarity::ADAFRUIT_SSD1680,
            );
            pair.set_pixel(0, 0, Gray4Color::White);
            pair.set_pixel(1, 0, Gray4Color::Light);
            pair.set_pixel(2, 0, Gray4Color::Dark);
            pair.set_pixel(3, 0, Gray4Color::Black);
        }

        // White: both planes clear.
        assert_eq!(plane_a[0] & 0x80, 0x00);
        assert_eq!(plane_b[0] & 0x80, 0x00);
        // Light: plane_a set, plane_b clear.
        assert_eq!(plane_a[0] & 0x40, 0x40);
        assert_eq!(plane_b[0] & 0x40, 0x00);
        // Dark: plane_a clear, plane_b set.
        assert_eq!(plane_a[0] & 0x20, 0x00);
        assert_eq!(plane_b[0] & 0x20, 0x20);
        // Black: both planes set.
        assert_eq!(plane_a[0] & 0x10, 0x10);
        assert_eq!(plane_b[0] & 0x10, 0x10);
    }

    /// Redrawing a pixel with a different color must not leave a stale bit on either plane.
    #[test]
    fn redrawing_a_pixel_clears_the_previous_colors_bits() {
        let mut plane_a = [0x00u8; 4];
        let mut plane_b = [0x00u8; 4];
        {
            let mut pair = GrayBufferPair::new(
                &mut plane_a,
                &mut plane_b,
                32,
                1,
                0,
                Gray4Polarity::ADAFRUIT_SSD1680,
            );
            pair.set_pixel(0, 0, Gray4Color::Black);
            pair.set_pixel(0, 0, Gray4Color::White);
        }

        assert_eq!(
            plane_a[0] & 0x80,
            0x00,
            "plane_a must be cleared back to White's code bit"
        );
        assert_eq!(
            plane_b[0] & 0x80,
            0x00,
            "plane_b must be cleared back to White's code bit"
        );
    }

    /// `GrayBufferPair` must work as an `embedded-graphics` `DrawTarget<Color = Gray4Color>`, not
    /// just through its own `set_pixel`.
    #[test]
    fn draw_target_routes_pixels_to_both_planes() {
        let mut plane_a = [0x00u8; 4];
        let mut plane_b = [0x00u8; 4];
        {
            let mut pair = GrayBufferPair::new(
                &mut plane_a,
                &mut plane_b,
                32,
                1,
                0,
                Gray4Polarity::ADAFRUIT_SSD1680,
            );
            Pixel(Point::new(0, 0), Gray4Color::Dark)
                .draw(&mut pair)
                .unwrap();
        }

        assert_eq!(plane_a[0] & 0x80, 0x00, "Dark leaves plane_a clear");
        assert_eq!(plane_b[0] & 0x80, 0x80, "Dark sets plane_b");
    }

    /// An inverted polarity must flip both planes' code bits, the same way
    /// [`PlanePolarity::UC8253`] flips `PageBufferPair`'s Black/White plane.
    #[test]
    fn inverted_polarity_flips_both_planes_code_bits() {
        let inverted = Gray4Polarity {
            plane_a_ink_is_set_bit: false,
            plane_b_ink_is_set_bit: false,
        };
        let mut plane_a = [0xFFu8; 4];
        let mut plane_b = [0xFFu8; 4];
        {
            let mut pair = GrayBufferPair::new(&mut plane_a, &mut plane_b, 32, 1, 0, inverted);
            pair.set_pixel(0, 0, Gray4Color::Black);
        }

        assert_eq!(
            plane_a[0] & 0x80,
            0x00,
            "inverted polarity clears the bit for a set code bit"
        );
        assert_eq!(
            plane_b[0] & 0x80,
            0x00,
            "inverted polarity clears the bit for a set code bit"
        );
    }

    /// `clear` must reset both planes to their own background byte under this pair's `polarity`.
    #[test]
    fn clear_resets_both_planes_to_their_own_background() {
        let mut plane_a = [0xFFu8; 4];
        let mut plane_b = [0xFFu8; 4];
        {
            let mut pair = GrayBufferPair::new(
                &mut plane_a,
                &mut plane_b,
                32,
                1,
                0,
                Gray4Polarity::ADAFRUIT_SSD1680,
            );
            pair.set_pixel(0, 0, Gray4Color::Black);
            pair.clear();
        }

        assert_eq!(
            plane_a, [0x00u8; 4],
            "plane_a must be back at its own background (0x00)"
        );
        assert_eq!(
            plane_b, [0x00u8; 4],
            "plane_b must be back at its own background (0x00)"
        );
    }

    /// `bounding_box` must reflect the shared width/height/`y_offset`, the same as
    /// `PageBufferPair`.
    #[test]
    fn bounding_box_matches_the_shared_page_geometry() {
        let mut plane_a = [0x00u8; 4];
        let mut plane_b = [0x00u8; 4];
        let pair = GrayBufferPair::new(
            &mut plane_a,
            &mut plane_b,
            32,
            4,
            8,
            Gray4Polarity::ADAFRUIT_SSD1680,
        );

        let bb = pair.bounding_box();
        assert_eq!(bb.top_left, Point::new(0, 8));
        assert_eq!(bb.size, Size::new(32, 4));
    }
}

// --- Bayer ordered dithering (backlog 1a) ---------------------------------------------------

mod dithering {
    use embedded_graphics_core::pixelcolor::{BinaryColor, Rgb888, RgbColor};
    use epdsi::graphics::dither::{dither_binary, dither_gray4, dither_seven, dither_tri};
    use epdsi::prelude::*;

    /// Every `(x, y)` position inside one 4x4 Bayer tile.
    fn tile() -> impl Iterator<Item = (u32, u32)> {
        (0..4u32).flat_map(|y| (0..4u32).map(move |x| (x, y)))
    }

    fn gray(v: u8) -> Rgb888 {
        Rgb888::new(v, v, v)
    }

    fn black_count(v: u8) -> usize {
        tile()
            .filter(|&(x, y)| dither_binary(x, y, gray(v)) == BinaryColor::On)
            .count()
    }

    #[test]
    fn binary_extremes_are_flat() {
        assert_eq!(black_count(0), 16);
        assert_eq!(black_count(255), 0);
    }

    #[test]
    fn binary_gradient_is_monotonic_and_mid_gray_is_half_black() {
        assert_eq!(black_count(128), 8);
        let mut prev = black_count(0);
        for v in 1..=255u8 {
            let n = black_count(v);
            assert!(n <= prev, "black count rose from {prev} to {n} at gray {v}");
            prev = n;
        }
    }

    #[test]
    fn binary_pattern_repeats_every_four_pixels() {
        for (x, y) in tile() {
            assert_eq!(
                dither_binary(x, y, gray(90)),
                dither_binary(x + 4, y + 8, gray(90))
            );
        }
    }

    fn gray4_level(c: Gray4Color) -> u32 {
        match c {
            Gray4Color::Black => 0,
            Gray4Color::Dark => 1,
            Gray4Color::Light => 2,
            Gray4Color::White => 3,
        }
    }

    #[test]
    fn gray4_extremes_and_exact_levels_are_flat() {
        for (x, y) in tile() {
            assert_eq!(dither_gray4(x, y, gray(0)), Gray4Color::Black);
            assert_eq!(dither_gray4(x, y, gray(85)), Gray4Color::Dark);
            assert_eq!(dither_gray4(x, y, gray(170)), Gray4Color::Light);
            assert_eq!(dither_gray4(x, y, gray(255)), Gray4Color::White);
        }
    }

    #[test]
    fn gray4_only_mixes_the_two_adjacent_levels() {
        for v in 0..=255u8 {
            let levels: Vec<u32> = tile()
                .map(|(x, y)| gray4_level(dither_gray4(x, y, gray(v))))
                .collect();
            let lo = *levels.iter().min().unwrap();
            let hi = *levels.iter().max().unwrap();
            assert!(hi - lo <= 1, "gray {v} mixed levels {lo} and {hi}");
        }
    }

    #[test]
    fn gray4_mean_level_follows_the_input() {
        let mean = |v: u8| -> u32 {
            tile()
                .map(|(x, y)| gray4_level(dither_gray4(x, y, gray(v))))
                .sum()
        };
        let mut prev = mean(0);
        for v in 1..=255u8 {
            let m = mean(v);
            assert!(m >= prev, "mean level fell at gray {v}");
            prev = m;
        }
    }

    #[test]
    fn tri_maps_palette_colors_exactly() {
        for (x, y) in tile() {
            assert_eq!(
                dither_tri(x, y, Rgb888::BLACK, Rgb888::RED),
                TriColor::Black
            );
            assert_eq!(
                dither_tri(x, y, Rgb888::WHITE, Rgb888::RED),
                TriColor::White
            );
            assert_eq!(dither_tri(x, y, Rgb888::RED, Rgb888::RED), TriColor::Accent);
            assert_eq!(
                dither_tri(x, y, Rgb888::YELLOW, Rgb888::YELLOW),
                TriColor::Accent
            );
        }
    }

    #[test]
    fn tri_gray_ramp_never_uses_the_accent() {
        for v in 0..=255u8 {
            for (x, y) in tile() {
                assert_ne!(
                    dither_tri(x, y, gray(v), Rgb888::RED),
                    TriColor::Accent,
                    "gray {v} at ({x},{y}) picked the accent ink"
                );
            }
        }
    }

    #[test]
    fn seven_maps_palette_colors_exactly() {
        let cases = [
            (Rgb888::BLACK, SevenColor::Black),
            (Rgb888::WHITE, SevenColor::White),
            (Rgb888::YELLOW, SevenColor::Yellow),
            (Rgb888::RED, SevenColor::Red),
            (Rgb888::BLUE, SevenColor::Blue),
            (Rgb888::GREEN, SevenColor::Green),
        ];
        for (rgb, want) in cases {
            for (x, y) in tile() {
                assert_eq!(dither_seven(x, y, rgb), want);
            }
        }
    }

    #[test]
    fn seven_never_returns_orange_or_clean() {
        for r in (0..=255u8).step_by(15) {
            for g in (0..=255u8).step_by(15) {
                for b in (0..=255u8).step_by(15) {
                    for (x, y) in tile() {
                        let c = dither_seven(x, y, Rgb888::new(r, g, b));
                        assert!(
                            !matches!(c, SevenColor::Orange | SevenColor::Clean),
                            "({r},{g},{b}) at ({x},{y}) gave {c:?}"
                        );
                    }
                }
            }
        }
    }

    /// A paged sweep must produce the same bytes as drawing the whole frame at once, because the
    /// dither depends only on the absolute `(x, y)` the caller draws at.
    #[test]
    fn paged_rendering_matches_full_frame_rendering() {
        const W: u32 = 122;
        const H: u32 = 20;
        const STRIDE: usize = 16;

        let source = |x: u32, y: u32| gray(((x * 2 + y * 9) % 256) as u8);

        let mut full = [0xFFu8; STRIDE * H as usize];
        {
            let mut buf = PageBuffer::new(&mut full, W, H, 0);
            for y in 0..H {
                for x in 0..W {
                    let black = dither_binary(x, y, source(x, y)) == BinaryColor::On;
                    buf.set_pixel(x, y, black);
                }
            }
        }

        let mut paged = [0xFFu8; STRIDE * H as usize];
        for (page, rows) in paged.chunks_mut(STRIDE * 8).enumerate() {
            let y_start = page as u32 * 8;
            let page_h = (rows.len() / STRIDE) as u32;
            let mut buf = PageBuffer::new(rows, W, page_h, y_start);
            for y in y_start..y_start + page_h {
                for x in 0..W {
                    let black = dither_binary(x, y, source(x, y)) == BinaryColor::On;
                    buf.set_pixel(x, y, black);
                }
            }
        }

        assert_eq!(full, paged);
    }
}

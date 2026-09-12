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

/// Regression tests for [`PageBufferPair`] / [`TriColor`] — the paired Black/White + accent
/// plane draw target Tri-Color panels use.
mod page_buffer_pair {
    use super::*;

    /// Each `TriColor` variant must write only the plane it owns, leaving the other at its
    /// background fill.
    #[test]
    fn each_color_writes_only_its_own_plane() {
        let mut bw = [0xFFu8; 4];
        let mut accent = [0xFFu8; 4];
        {
            let mut pair = PageBufferPair::new(&mut bw, &mut accent, 32, 1, 0);
            pair.set_pixel(0, 0, TriColor::Black);
            pair.set_pixel(1, 0, TriColor::Accent);
            pair.set_pixel(2, 0, TriColor::White);
        }

        // Black: bw bit cleared, accent untouched (still background).
        assert_eq!(bw[0] & 0x80, 0x00);
        assert_eq!(accent[0] & 0x80, 0x80);
        // Accent: accent bit cleared, bw untouched (still background).
        assert_eq!(bw[0] & 0x40, 0x40);
        assert_eq!(accent[0] & 0x40, 0x00);
        // White: both planes at background.
        assert_eq!(bw[0] & 0x20, 0x20);
        assert_eq!(accent[0] & 0x20, 0x20);
    }

    /// Redrawing a pixel with a different color must not leave a stale bit on the plane the
    /// previous color used — otherwise an accent pixel painted over with black would still
    /// render red/yellow underneath the black ink.
    #[test]
    fn redrawing_a_pixel_clears_the_previous_colors_plane() {
        let mut bw = [0xFFu8; 4];
        let mut accent = [0xFFu8; 4];
        {
            let mut pair = PageBufferPair::new(&mut bw, &mut accent, 32, 1, 0);
            pair.set_pixel(0, 0, TriColor::Accent);
            pair.set_pixel(0, 0, TriColor::Black);
        }

        assert_eq!(bw[0] & 0x80, 0x00, "black bit should be set on bw plane");
        assert_eq!(
            accent[0] & 0x80,
            0x80,
            "accent plane must be cleared back to background, not left set from the earlier draw"
        );
    }

    /// `PageBufferPair` must work as an `embedded-graphics` `DrawTarget<Color = TriColor>`, not
    /// just through its own `set_pixel`.
    #[test]
    fn draw_target_routes_pixels_to_the_correct_plane() {
        let mut bw = [0xFFu8; 4];
        let mut accent = [0xFFu8; 4];
        {
            let mut pair = PageBufferPair::new(&mut bw, &mut accent, 32, 1, 0);
            Pixel(Point::new(1, 0), TriColor::Accent)
                .draw(&mut pair)
                .unwrap();
        }

        assert_eq!(accent[0] & 0x40, 0x00);
        assert_eq!(bw[0] & 0x40, 0x40);
    }

    /// `bounding_box` must reflect the shared width/height/`y_offset`, the same as a plain
    /// `PageBuffer`.
    #[test]
    fn bounding_box_matches_the_shared_page_geometry() {
        let mut bw = [0xFFu8; 4];
        let mut accent = [0xFFu8; 4];
        let pair = PageBufferPair::new(&mut bw, &mut accent, 32, 4, 8);

        let bb = pair.bounding_box();
        assert_eq!(bb.top_left, Point::new(0, 8));
        assert_eq!(bb.size, Size::new(32, 4));
    }
}

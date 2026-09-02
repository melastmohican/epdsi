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

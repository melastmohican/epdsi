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

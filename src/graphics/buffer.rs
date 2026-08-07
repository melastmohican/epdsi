//! Sub-region stack page buffer for low-RAM microcontrollers.

#[cfg(feature = "graphics")]
use embedded_graphics_core::{
    geometry::{Point, Size},
    pixelcolor::BinaryColor,
    prelude::*,
    primitives::Rectangle,
};

/// Display rotation options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DisplayRotation {
    /// 0 degrees (default)
    #[default]
    Rotate0,
    /// 90 degrees clockwise
    Rotate90,
    /// 180 degrees clockwise
    Rotate180,
    /// 270 degrees clockwise
    Rotate270,
}

/// A memory-constrained stack sub-region buffer for paged rendering.
pub struct PageBuffer<'a> {
    buffer: &'a mut [u8],
    width: u32,
    height: u32,
    y_offset: u32,
    rotation: DisplayRotation,
}

impl<'a> PageBuffer<'a> {
    /// Creates a new `PageBuffer` wrapping a mutable slice.
    pub fn new(buffer: &'a mut [u8], width: u32, height: u32, y_offset: u32) -> Self {
        Self {
            buffer,
            width,
            height,
            y_offset,
            rotation: DisplayRotation::Rotate0,
        }
    }

    /// Sets the rotation of the display buffer.
    pub fn set_rotation(&mut self, rotation: DisplayRotation) {
        self.rotation = rotation;
    }

    /// Returns current display rotation.
    pub fn rotation(&self) -> DisplayRotation {
        self.rotation
    }

    /// Clears page buffer with raw byte value (0xFF for White, 0x00 for Black).
    pub fn clear_byte(&mut self, val: u8) {
        self.buffer.fill(val);
    }

    /// Access inner slice raw byte data.
    pub fn as_slice(&self) -> &[u8] {
        self.buffer
    }

    /// Access inner slice mutable raw byte data.
    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        self.buffer
    }

    /// Returns target display width in pixels.
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Returns current page slice height in pixels.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Returns global Y pixel offset for current page.
    pub fn y_offset(&self) -> u32 {
        self.y_offset
    }

    /// Sets a pixel bit directly in the 1-bit per pixel buffer.
    ///
    /// Coordinate (x, y) is in absolute display space.
    pub fn set_pixel(&mut self, x: u32, y: u32, black: bool) {
        let (mapped_x, mapped_y) = match self.rotation {
            DisplayRotation::Rotate0 => (x, y),
            DisplayRotation::Rotate90 => (self.width.saturating_sub(1).saturating_sub(y), x),
            DisplayRotation::Rotate180 => (
                self.width.saturating_sub(1).saturating_sub(x),
                self.height.saturating_sub(1).saturating_sub(y),
            ),
            DisplayRotation::Rotate270 => (y, self.height.saturating_sub(1).saturating_sub(x)),
        };

        if mapped_x >= self.width || mapped_y < self.y_offset || mapped_y >= self.y_offset + self.height {
            return;
        }

        let local_y = mapped_y - self.y_offset;
        let index = ((local_y * self.width + mapped_x) / 8) as usize;
        let bit = 7 - (mapped_x % 8);

        if index < self.buffer.len() {
            if black {
                self.buffer[index] &= !(1 << bit);
            } else {
                self.buffer[index] |= 1 << bit;
            }
        }
    }
}

#[cfg(feature = "graphics")]
impl<'a> Dimensions for PageBuffer<'a> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(
            Point::new(0, self.y_offset as i32),
            Size::new(self.width, self.height),
        )
    }
}

#[cfg(feature = "graphics")]
impl<'a> DrawTarget for PageBuffer<'a> {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 {
                let x = point.x as u32;
                let y = point.y as u32;
                let black = match color {
                    BinaryColor::On => true,
                    BinaryColor::Off => false,
                };
                self.set_pixel(x, y, black);
            }
        }
        Ok(())
    }
}

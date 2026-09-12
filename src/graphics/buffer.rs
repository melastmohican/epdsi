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
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
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
    /// Row stride in bytes, rounded up to a whole byte. Panels whose width is not a multiple of
    /// 8 (such as the 122 px GDEM0213B74 and ZJY122250) address RAM in whole bytes, so a row
    /// occupies `width.div_ceil(8)` bytes and the trailing bits are off-panel padding.
    stride: usize,
    y_offset: u32,
    rotation: DisplayRotation,
}

impl<'a> PageBuffer<'a> {
    /// Creates a new `PageBuffer` wrapping a mutable slice.
    /// `buffer` must be at least `width.div_ceil(8) * height` bytes long.
    pub fn new(buffer: &'a mut [u8], width: u32, height: u32, y_offset: u32) -> Self {
        Self {
            buffer,
            width,
            height,
            stride: width.div_ceil(8) as usize,
            y_offset,
            rotation: DisplayRotation::Rotate0,
        }
    }

    /// Returns the row stride in bytes (`width` rounded up to a whole byte).
    pub fn stride(&self) -> usize {
        self.stride
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

        if mapped_x >= self.width
            || mapped_y < self.y_offset
            || mapped_y >= self.y_offset + self.height
        {
            return;
        }

        let local_y = mapped_y - self.y_offset;
        let index = local_y as usize * self.stride + (mapped_x / 8) as usize;
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
#[cfg_attr(docsrs, doc(cfg(feature = "graphics")))]
impl<'a> Dimensions for PageBuffer<'a> {
    fn bounding_box(&self) -> Rectangle {
        Rectangle::new(
            Point::new(0, self.y_offset as i32),
            Size::new(self.width, self.height),
        )
    }
}

#[cfg(feature = "graphics")]
#[cfg_attr(docsrs, doc(cfg(feature = "graphics")))]
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

/// The three-state palette a Tri-Color panel's two RAM planes render together: Black/White plus
/// one accent ink, wired to whichever physical color the panel's second plane carries (red or
/// yellow — see [`ColorChannel::RedYellow`](crate::traits::ColorChannel::RedYellow)).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum TriColor {
    /// White — no ink in either plane.
    #[default]
    White,
    /// Black ink, written to the Black/White plane.
    Black,
    /// Accent ink (red or yellow, depending on the panel), written to the second plane.
    Accent,
}

#[cfg(feature = "graphics")]
#[cfg_attr(docsrs, doc(cfg(feature = "graphics")))]
impl PixelColor for TriColor {
    type Raw = ();
}

/// Which raw RAM bit means "ink" on each of a Tri-Color panel's two planes.
///
/// Panels genuinely disagree here, which is why this is a required constructor argument rather
/// than a hardcoded assumption — the single most common porting mistake on these panels (see the
/// `epdsi` hardware examples' "ink polarity" notes) is assuming one panel's convention applies to
/// another's.
///
/// - **SSD1680 / SSD1681** (`GDEY0266Z90`, `GDEM0154Z90`): the Black/White plane is normal — a
///   *cleared* bit is black — but the accent plane is inverted: a **set** bit is red/yellow.
///   Use [`Self::SSD168X`].
/// - **UC8253** (`SE0352N14TNGA0`): *both* planes are inverted — a set bit is ink on either one.
///   Use [`Self::UC8253`].
///
/// Getting this backwards does not error; it silently paints the background in ink and the
/// intended content in background, on hardware, which is why it is worth pinning as a named
/// constant per controller rather than four bare booleans scattered through call sites.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlanePolarity {
    /// `true` if a *set* bit (rather than a cleared one) means black ink on the Black/White plane.
    pub bw_ink_is_set_bit: bool,
    /// `true` if a *set* bit (rather than a cleared one) means accent ink on the second plane.
    pub accent_ink_is_set_bit: bool,
}

impl PlanePolarity {
    /// The SSD1680/SSD1681 Tri-Color convention: Black/White plane normal, accent plane inverted.
    /// Verified on hardware against `GDEY0266Z90` and `GDEM0154Z90`.
    pub const SSD168X: Self = Self {
        bw_ink_is_set_bit: false,
        accent_ink_is_set_bit: true,
    };

    /// The UC8253 convention: both planes inverted. Verified on hardware against `SE0352N14TNGA0`.
    pub const UC8253: Self = Self {
        bw_ink_is_set_bit: true,
        accent_ink_is_set_bit: true,
    };

    /// The background (non-ink) fill byte for the Black/White plane under this polarity —
    /// `0xFF` if ink is the cleared bit, `0x00` if ink is the set bit.
    pub const fn bw_background_byte(&self) -> u8 {
        if self.bw_ink_is_set_bit { 0x00 } else { 0xFF }
    }

    /// The background (non-ink) fill byte for the accent plane under this polarity.
    pub const fn accent_background_byte(&self) -> u8 {
        if self.accent_ink_is_set_bit { 0x00 } else { 0xFF }
    }
}

/// A pair of [`PageBuffer`]s addressing a Tri-Color panel's Black/White and accent (red/yellow)
/// RAM planes together as one `DrawTarget<Color = TriColor>`. One `embedded-graphics` draw call
/// routes each pixel to the correct plane, instead of needing two separate [`render_paged`]
/// passes with a `BinaryColor` closure apiece.
///
/// [`render_paged`]: crate::graphics::paged::render_paged
pub struct PageBufferPair<'a> {
    bw: PageBuffer<'a>,
    accent: PageBuffer<'a>,
    polarity: PlanePolarity,
}

impl<'a> PageBufferPair<'a> {
    /// Creates a new `PageBufferPair` wrapping the Black/White and accent plane buffers for one
    /// page. Both buffers must be at least `width.div_ceil(8) * height` bytes long.
    ///
    /// `polarity` must match the panel's actual RAM plane behavior — see [`PlanePolarity`]'s
    /// docs for the two conventions `epdsi` ships panels for.
    pub fn new(
        bw_buffer: &'a mut [u8],
        accent_buffer: &'a mut [u8],
        width: u32,
        height: u32,
        y_offset: u32,
        polarity: PlanePolarity,
    ) -> Self {
        Self {
            bw: PageBuffer::new(bw_buffer, width, height, y_offset),
            accent: PageBuffer::new(accent_buffer, width, height, y_offset),
            polarity,
        }
    }

    /// Sets the rotation on both planes together.
    pub fn set_rotation(&mut self, rotation: DisplayRotation) {
        self.bw.set_rotation(rotation);
        self.accent.set_rotation(rotation);
    }

    /// Returns the current rotation.
    pub fn rotation(&self) -> DisplayRotation {
        self.bw.rotation()
    }

    /// Access the Black/White plane buffer.
    pub fn bw(&self) -> &PageBuffer<'a> {
        &self.bw
    }

    /// Access the accent (red/yellow) plane buffer.
    pub fn accent(&self) -> &PageBuffer<'a> {
        &self.accent
    }

    /// Sets one pixel to `color`, clearing the plane `color` does not use so a pixel redrawn
    /// with a different color does not leave a stale bit behind on the other plane.
    ///
    /// Coordinate (x, y) is in absolute display space, same as [`PageBuffer::set_pixel`]. The
    /// raw bit written on each plane is derived from `color` together with the `polarity` this
    /// pair was constructed with — [`PageBuffer::set_pixel`]'s own `black` argument means
    /// "clear the bit," so it is only equal to "this plane's ink" when that plane's ink is *not*
    /// the set bit; the `!=` below is exactly that XOR.
    pub fn set_pixel(&mut self, x: u32, y: u32, color: TriColor) {
        let (bw_ink, accent_ink) = match color {
            TriColor::White => (false, false),
            TriColor::Black => (true, false),
            TriColor::Accent => (false, true),
        };
        self.bw
            .set_pixel(x, y, bw_ink != self.polarity.bw_ink_is_set_bit);
        self.accent
            .set_pixel(x, y, accent_ink != self.polarity.accent_ink_is_set_bit);
    }
}

#[cfg(feature = "graphics")]
#[cfg_attr(docsrs, doc(cfg(feature = "graphics")))]
impl<'a> Dimensions for PageBufferPair<'a> {
    fn bounding_box(&self) -> Rectangle {
        self.bw.bounding_box()
    }
}

#[cfg(feature = "graphics")]
#[cfg_attr(docsrs, doc(cfg(feature = "graphics")))]
impl<'a> DrawTarget for PageBufferPair<'a> {
    type Color = TriColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 {
                self.set_pixel(point.x as u32, point.y as u32, color);
            }
        }
        Ok(())
    }
}

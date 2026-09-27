//! GxEPD2-style closure-based paged drawing engine.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;

#[cfg(feature = "blocking")]
use embedded_hal::digital::InputPin as Ssd1677Wait;
use embedded_hal::digital::{InputPin, OutputPin};
#[cfg(feature = "blocking")]
use embedded_hal::spi::SpiDevice;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait as Ssd1677Wait;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::controllers::{Ssd1677Controller, Ssd1677RefreshMode};
use crate::driver::EpdDriver;
use crate::graphics::buffer::{
    Gray4Polarity, GrayBufferPair, PageBuffer, PageBufferPair, PlanePolarity,
};
use crate::traits::{ColorChannel, EpdController, EpdPanel};

/// Executes a GxEPD2-style paged rendering loop over display sub-regions.
///
/// Sweeps through display memory page-by-page using a small stack buffer. The pixel-drawing
/// closure `draw_fn` stays synchronous in both build modes — `embedded-graphics-core`'s
/// `DrawTarget` has no async counterpart — only the I/O between pages (`set_window`/
/// `set_cursor`/`write_frame`/`refresh`) is `.await`ed in async mode.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
pub async fn render_paged<BUS, CONTROLLER, PANEL, DELAY, F>(
    driver: &mut EpdDriver<BUS, CONTROLLER, PANEL>,
    delay: &mut DELAY,
    channel: ColorChannel,
    page_buffer: &mut [u8],
    page_height: u32,
    clear_byte: u8,
    mut draw_fn: F,
) -> Result<(), CONTROLLER::Error>
where
    CONTROLLER: EpdController<BUS>,
    PANEL: EpdPanel,
    DELAY: DelayNs,
    F: FnMut(&mut PageBuffer),
{
    let width = PANEL::WIDTH;
    let height = PANEL::HEIGHT;

    let total_pages = height.div_ceil(page_height);

    for page_idx in 0..total_pages {
        let y_start = page_idx * page_height;
        let y_end = (y_start + page_height).min(height) - 1;
        let current_page_height = y_end - y_start + 1;

        let required_bytes = width.div_ceil(8) as usize * current_page_height as usize;

        // Reset page buffer contents
        page_buffer[..required_bytes].fill(clear_byte);

        // Instantiate page sub-region buffer wrapper
        let mut page_buf = PageBuffer::new(
            &mut page_buffer[..required_bytes],
            width,
            current_page_height,
            y_start,
        );

        // Invoke user drawing closure
        draw_fn(&mut page_buf);

        // Configure hardware display window and write page chunk to RAM
        driver.set_window(0, y_start, width - 1, y_end).await?;
        driver.set_cursor(0, y_start).await?;
        driver.write_frame(channel, page_buf.as_slice()).await?;
    }

    // Trigger physical display update
    driver.refresh(delay).await
}

/// Tri-Color counterpart of [`render_paged`]: sweeps the panel page-by-page like `render_paged`
/// does, but hands the drawing closure a [`PageBufferPair`] so one `embedded-graphics` pass can
/// address both the Black/White and accent (red/yellow) planes together, then writes each plane
/// to its own channel before advancing to the next page.
///
/// `accent_channel` is the panel's second-plane channel — [`ColorChannel::RedYellow`] for every
/// Tri-Color panel `epdsi` ships. `page_buffers` is `(bw_page_buffer, accent_page_buffer)` —
/// bundled into one tuple to keep the parameter count in line with `render_paged`'s — and each
/// must be sized for one page, the same as `render_paged`'s single buffer. `polarity` must match
/// the panel wired up — see [`PlanePolarity`]'s docs; there is no safe default, since panels
/// genuinely disagree. Each plane's background fill byte is derived from `polarity` rather than
/// taken as a parameter, so the fill can't drift out of sync with the polarity used to draw it.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
pub async fn render_paged_tri_color<BUS, CONTROLLER, PANEL, DELAY, F>(
    driver: &mut EpdDriver<BUS, CONTROLLER, PANEL>,
    delay: &mut DELAY,
    accent_channel: ColorChannel,
    page_buffers: (&mut [u8], &mut [u8]),
    polarity: PlanePolarity,
    page_height: u32,
    mut draw_fn: F,
) -> Result<(), CONTROLLER::Error>
where
    CONTROLLER: EpdController<BUS>,
    PANEL: EpdPanel,
    DELAY: DelayNs,
    F: FnMut(&mut PageBufferPair),
{
    let (bw_page_buffer, accent_page_buffer) = page_buffers;
    let width = PANEL::WIDTH;
    let height = PANEL::HEIGHT;

    let total_pages = height.div_ceil(page_height);

    for page_idx in 0..total_pages {
        let y_start = page_idx * page_height;
        let y_end = (y_start + page_height).min(height) - 1;
        let current_page_height = y_end - y_start + 1;

        let required_bytes = width.div_ceil(8) as usize * current_page_height as usize;

        // Reset both plane buffers to their own background fill — the two can differ (see
        // `PlanePolarity`), which a single shared `clear_byte` could not express correctly.
        bw_page_buffer[..required_bytes].fill(polarity.bw_background_byte());
        accent_page_buffer[..required_bytes].fill(polarity.accent_background_byte());

        // Instantiate the paired page sub-region buffer wrapper
        let mut page_buf = PageBufferPair::new(
            &mut bw_page_buffer[..required_bytes],
            &mut accent_page_buffer[..required_bytes],
            width,
            current_page_height,
            y_start,
            polarity,
        );

        // Invoke user drawing closure
        draw_fn(&mut page_buf);

        // Configure hardware display window once, then write each plane to its own channel —
        // the window/cursor registers are shared state, selected per write by `channel` alone.
        driver.set_window(0, y_start, width - 1, y_end).await?;
        driver.set_cursor(0, y_start).await?;
        driver
            .write_frame(ColorChannel::BlackWhite, page_buf.bw().as_slice())
            .await?;
        driver.set_cursor(0, y_start).await?;
        driver
            .write_frame(accent_channel, page_buf.accent().as_slice())
            .await?;
    }

    // Trigger physical display update
    driver.refresh(delay).await
}

/// Gray4 counterpart of [`render_paged`]: sweeps the panel page-by-page like `render_paged` does,
/// but hands the drawing closure a [`GrayBufferPair`] so one `embedded-graphics` pass can address
/// both RAM planes together as a 4-level grayscale code, then writes each plane to its own channel
/// before advancing to the next page.
///
/// For panels configured with Adafruit_EPD-style Gray4 mode (see
/// [`Gray4Registers`](crate::traits::Gray4Registers)/[`EpdPanel::GRAY4`]
/// and [`Ssd168xRefreshMode::Gray4`](crate::controllers::Ssd168xRefreshMode::Gray4)) — the
/// controller's `refresh_mode` must already be set to `Gray4` before calling this, same as any
/// other refresh mode/render function pairing.
///
/// `page_buffers` is `(plane_a_page_buffer, plane_b_page_buffer)` — the Black/White and
/// Red/Yellow plane buffers respectively, each sized for one page, same as
/// [`render_paged_tri_color`]. Unlike that function, the two RAM channels are fixed
/// (`ColorChannel::BlackWhite`/`ColorChannel::RedYellow`) rather than taking an `accent_channel`
/// parameter, since Gray4 mode always reuses exactly those two planes as its bit-planes.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
pub async fn render_paged_gray4<BUS, CONTROLLER, PANEL, DELAY, F>(
    driver: &mut EpdDriver<BUS, CONTROLLER, PANEL>,
    delay: &mut DELAY,
    page_buffers: (&mut [u8], &mut [u8]),
    polarity: Gray4Polarity,
    page_height: u32,
    mut draw_fn: F,
) -> Result<(), CONTROLLER::Error>
where
    CONTROLLER: EpdController<BUS>,
    PANEL: EpdPanel,
    DELAY: DelayNs,
    F: FnMut(&mut GrayBufferPair),
{
    let (plane_a_page_buffer, plane_b_page_buffer) = page_buffers;
    let width = PANEL::WIDTH;
    let height = PANEL::HEIGHT;

    let total_pages = height.div_ceil(page_height);

    for page_idx in 0..total_pages {
        let y_start = page_idx * page_height;
        let y_end = (y_start + page_height).min(height) - 1;
        let current_page_height = y_end - y_start + 1;

        let required_bytes = width.div_ceil(8) as usize * current_page_height as usize;

        plane_a_page_buffer[..required_bytes].fill(polarity.plane_a_background_byte());
        plane_b_page_buffer[..required_bytes].fill(polarity.plane_b_background_byte());

        let mut page_buf = GrayBufferPair::new(
            &mut plane_a_page_buffer[..required_bytes],
            &mut plane_b_page_buffer[..required_bytes],
            width,
            current_page_height,
            y_start,
            polarity,
        );

        // Invoke user drawing closure
        draw_fn(&mut page_buf);

        // Configure hardware display window once, then write each plane to its own channel —
        // the window/cursor registers are shared state, selected per write by `channel` alone.
        driver.set_window(0, y_start, width - 1, y_end).await?;
        driver.set_cursor(0, y_start).await?;
        driver
            .write_frame(ColorChannel::BlackWhite, page_buf.plane_a().as_slice())
            .await?;
        driver.set_cursor(0, y_start).await?;
        driver
            .write_frame(ColorChannel::RedYellow, page_buf.plane_b().as_slice())
            .await?;
    }

    // Trigger physical display update
    driver.refresh(delay).await
}

/// Two-pass Gray4 counterpart of [`render_paged_gray4`], for
/// [`Ssd1677Controller`]/[`GDEQ0426T82`](crate::panels::GDEQ0426T82) — the only Gray4-capable
/// controller in this crate whose refresh sequence needs a baseline monochrome pass before the
/// custom LUT can render intermediate gray levels correctly (ported from
/// `Adafruit_SSD1677::update()`'s grayscale branch). Unlike `render_paged_gray4`, which is generic
/// over any [`EpdController`], this function is concrete to `Ssd1677Controller` because the
/// two-pass sequencing needs [`Ssd1677Controller::reload_gray4_lut`] and
/// [`Ssd1677RefreshMode`] switching mid-flow — neither expressible through the generic trait.
///
/// Configure the controller with `.with_gray4(PANEL::GRAY4)` before calling this; the refresh
/// mode is managed internally (set to [`Ssd1677RefreshMode::Gray4Preclear`] then
/// [`Ssd1677RefreshMode::Gray4`]) and left on `Gray4` afterward.
///
/// # Sequencing
/// 1. Sweep every page, drawing `draw_fn` and writing the resulting Black/White plane to *both*
///    the `BlackWhite` and `RedYellow` channels (same bytes) — this "preclears" the panel to the
///    final image's black/white split before any gray waveform runs.
/// 2. Trigger a [`Ssd1677RefreshMode::Gray4Preclear`] refresh — one whole-panel OTP-LUT refresh
///    with the Red/Yellow plane bypassed.
/// 3. Reload the Gray4 LUT and voltage registers via
///    [`Ssd1677Controller::reload_gray4_lut`] — the preclear refresh's OTP LUT load overwrites the
///    custom LUT uploaded during `init`.
/// 4. Sweep every page a **second time**, re-invoking `draw_fn`, writing the real Black/White
///    (LSB) and Red/Yellow (MSB) planes.
/// 5. Trigger a [`Ssd1677RefreshMode::Gray4`] refresh — the final pass with the custom LUT.
///
/// # `draw_fn` is called twice per page
///
/// Page buffers are not retained across the frame — the whole point of paging is bounded memory
/// — so the second sweep reconstructs each page's pixel data by calling `draw_fn` again rather
/// than caching it. **`draw_fn` must be deterministic**: drawing the same page twice must produce
/// the same bytes both times, or the final image will not match what the preclear pass set as the
/// baseline. A closure reading from a stable source (a framebuffer, an image, `embedded-graphics`
/// primitives) satisfies this; one with side effects that change what gets drawn between calls
/// does not.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::type_complexity)]
pub async fn render_paged_gray4_preclear<SPI, DC, RST, BUSY, PANEL, DELAY, F>(
    driver: &mut EpdDriver<SpiBusWrapper<SPI, DC, RST, BUSY>, Ssd1677Controller, PANEL>,
    delay: &mut DELAY,
    page_buffers: (&mut [u8], &mut [u8]),
    polarity: Gray4Polarity,
    page_height: u32,
    mut draw_fn: F,
) -> Result<(), EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin + Ssd1677Wait,
    PANEL: EpdPanel,
    DELAY: DelayNs,
    F: FnMut(&mut GrayBufferPair),
{
    let (plane_a_page_buffer, plane_b_page_buffer) = page_buffers;
    let width = PANEL::WIDTH;
    let height = PANEL::HEIGHT;
    let total_pages = height.div_ceil(page_height);

    // Pass 1: preclear. Same Black/White content goes to both channels.
    for page_idx in 0..total_pages {
        let y_start = page_idx * page_height;
        let y_end = (y_start + page_height).min(height) - 1;
        let current_page_height = y_end - y_start + 1;
        let required_bytes = width.div_ceil(8) as usize * current_page_height as usize;

        plane_a_page_buffer[..required_bytes].fill(polarity.plane_a_background_byte());
        plane_b_page_buffer[..required_bytes].fill(polarity.plane_b_background_byte());
        let mut page_buf = GrayBufferPair::new(
            &mut plane_a_page_buffer[..required_bytes],
            &mut plane_b_page_buffer[..required_bytes],
            width,
            current_page_height,
            y_start,
            polarity,
        );
        draw_fn(&mut page_buf);

        driver.set_window(0, y_start, width - 1, y_end).await?;
        driver.set_cursor(0, y_start).await?;
        driver
            .write_frame(ColorChannel::BlackWhite, page_buf.plane_a().as_slice())
            .await?;
        driver.set_cursor(0, y_start).await?;
        driver
            .write_frame(ColorChannel::RedYellow, page_buf.plane_a().as_slice())
            .await?;
    }

    driver
        .controller_mut()
        .set_refresh_mode(Ssd1677RefreshMode::Gray4Preclear);
    driver.refresh(delay).await?;

    {
        let (bus, controller) = driver.split_mut();
        controller.reload_gray4_lut(bus).await?;
    }

    driver
        .controller_mut()
        .set_refresh_mode(Ssd1677RefreshMode::Gray4);

    // Pass 2: the real image.
    for page_idx in 0..total_pages {
        let y_start = page_idx * page_height;
        let y_end = (y_start + page_height).min(height) - 1;
        let current_page_height = y_end - y_start + 1;
        let required_bytes = width.div_ceil(8) as usize * current_page_height as usize;

        plane_a_page_buffer[..required_bytes].fill(polarity.plane_a_background_byte());
        plane_b_page_buffer[..required_bytes].fill(polarity.plane_b_background_byte());
        let mut page_buf = GrayBufferPair::new(
            &mut plane_a_page_buffer[..required_bytes],
            &mut plane_b_page_buffer[..required_bytes],
            width,
            current_page_height,
            y_start,
            polarity,
        );
        draw_fn(&mut page_buf);

        driver.set_window(0, y_start, width - 1, y_end).await?;
        driver.set_cursor(0, y_start).await?;
        driver
            .write_frame(ColorChannel::BlackWhite, page_buf.plane_a().as_slice())
            .await?;
        driver.set_cursor(0, y_start).await?;
        driver
            .write_frame(ColorChannel::RedYellow, page_buf.plane_b().as_slice())
            .await?;
    }

    driver.refresh(delay).await
}

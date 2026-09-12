//! GxEPD2-style closure-based paged drawing engine.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;

use crate::driver::EpdDriver;
use crate::graphics::buffer::{PageBuffer, PageBufferPair};
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
        driver
            .write_frame(channel, page_buf.as_slice())
            .await?;
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
/// must be sized for one page, the same as `render_paged`'s single buffer.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
pub async fn render_paged_tri_color<BUS, CONTROLLER, PANEL, DELAY, F>(
    driver: &mut EpdDriver<BUS, CONTROLLER, PANEL>,
    delay: &mut DELAY,
    accent_channel: ColorChannel,
    page_buffers: (&mut [u8], &mut [u8]),
    page_height: u32,
    clear_byte: u8,
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

        // Reset both plane buffers to the same background fill
        bw_page_buffer[..required_bytes].fill(clear_byte);
        accent_page_buffer[..required_bytes].fill(clear_byte);

        // Instantiate the paired page sub-region buffer wrapper
        let mut page_buf = PageBufferPair::new(
            &mut bw_page_buffer[..required_bytes],
            &mut accent_page_buffer[..required_bytes],
            width,
            current_page_height,
            y_start,
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

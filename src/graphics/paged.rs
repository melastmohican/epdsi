//! GxEPD2-style closure-based paged drawing engine.

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;

use crate::driver::EpdDriver;
use crate::graphics::buffer::PageBuffer;
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

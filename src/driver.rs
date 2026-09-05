//! Display driver orchestrator and builder implementation.

use core::marker::PhantomData;
#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;

use crate::bus::ValidationError;
use crate::traits::{ColorChannel, EpdController, EpdPanel};

/// Primary display driver orchestrating physical communications, controller logic, and panel dimensions.
pub struct EpdDriver<BUS, CONTROLLER, PANEL> {
    bus: BUS,
    controller: CONTROLLER,
    _panel: PhantomData<PANEL>,
    /// Last window handed to (or defaulted for) `set_window`, as `(x_start, y_start, x_end,
    /// y_end)`. `write_frame`'s buffer is addressed against whatever window is currently active
    /// on the hardware — a page in `render_paged`, or the full panel by default — not the panel's
    /// full size, so this is what `required_bytes` validates against.
    window: (u32, u32, u32, u32),
}

impl<BUS, CONTROLLER, PANEL> EpdDriver<BUS, CONTROLLER, PANEL>
where
    CONTROLLER: EpdController<BUS>,
    PANEL: EpdPanel,
{
    /// Creates a new `EpdDriver` wrapping bus and controller implementations.
    pub fn new(bus: BUS, controller: CONTROLLER) -> Self {
        Self {
            bus,
            controller,
            _panel: PhantomData,
            window: (0, 0, PANEL::WIDTH - 1, PANEL::HEIGHT - 1),
        }
    }

    /// Access immutable reference to communication bus.
    pub fn bus(&self) -> &BUS {
        &self.bus
    }

    /// Access mutable reference to communication bus.
    pub fn bus_mut(&mut self) -> &mut BUS {
        &mut self.bus
    }

    /// Access immutable reference to controller logic.
    pub fn controller(&self) -> &CONTROLLER {
        &self.controller
    }

    /// Access mutable reference to controller logic.
    pub fn controller_mut(&mut self) -> &mut CONTROLLER {
        &mut self.controller
    }

    /// Access simultaneous mutable references to both communication bus and controller logic.
    pub fn split_mut(&mut self) -> (&mut BUS, &mut CONTROLLER) {
        (&mut self.bus, &mut self.controller)
    }

    /// Returns display panel width in pixels.
    pub fn width(&self) -> u32 {
        PANEL::WIDTH
    }

    /// Returns display panel height in pixels.
    pub fn height(&self) -> u32 {
        PANEL::HEIGHT
    }

    /// Minimum buffer length `write_frame` requires for `channel` against the currently active
    /// window — the full panel by default, or one page's worth of rows mid-`render_paged`.
    ///
    /// [`ColorChannel::Color7`] packs two 4-bit pixels per byte (see
    /// [`SevenColor::pack`](crate::traits::SevenColor::pack)); every other channel is 1 bit per
    /// pixel, row-byte-aligned like [`Self::clear_frame`].
    fn required_bytes(&self, channel: ColorChannel) -> usize {
        let (x_start, y_start, x_end, y_end) = self.window;
        let width = (x_end - x_start + 1) as usize;
        let height = (y_end - y_start + 1) as usize;
        match channel {
            ColorChannel::Color7(_) => (width * height).div_ceil(2),
            _ => width.div_ceil(8) * height,
        }
    }
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<BUS, CONTROLLER, PANEL> EpdDriver<BUS, CONTROLLER, PANEL>
where
    CONTROLLER: EpdController<BUS>,
    PANEL: EpdPanel,
{
    /// Initializes hardware reset, command registers, and panel configuration.
    pub async fn init<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<(), CONTROLLER::Error> {
        self.controller.init_sequence(&mut self.bus, delay).await
    }

    /// Sets display RAM active window boundaries.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::InvalidWindow`] (via `CONTROLLER::Error`) if the range is
    /// inverted or falls outside the panel's declared `PANEL::WIDTH`/`PANEL::HEIGHT`, without
    /// sending anything to the bus.
    pub async fn set_window(
        &mut self,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), CONTROLLER::Error> {
        if x_start > x_end
            || y_start > y_end
            || x_end >= PANEL::WIDTH
            || y_end >= PANEL::HEIGHT
        {
            return Err(CONTROLLER::Error::from(ValidationError::InvalidWindow));
        }
        self.controller
            .set_window(&mut self.bus, x_start, y_start, x_end, y_end)
            .await?;
        self.window = (x_start, y_start, x_end, y_end);
        Ok(())
    }

    /// Sets display RAM cursor position.
    pub async fn set_cursor(&mut self, x: u32, y: u32) -> Result<(), CONTROLLER::Error> {
        self.controller.set_cursor(&mut self.bus, x, y).await
    }

    /// Writes raw slice data into display controller RAM.
    ///
    /// # Errors
    ///
    /// Returns [`ValidationError::BufferTooSmall`] (via `CONTROLLER::Error`) if `data` is
    /// shorter than the channel requires for the panel's dimensions, without sending anything
    /// to the bus.
    pub async fn write_frame(
        &mut self,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), CONTROLLER::Error> {
        let required = self.required_bytes(channel);
        if data.len() < required {
            return Err(CONTROLLER::Error::from(ValidationError::BufferTooSmall {
                required,
                provided: data.len(),
            }));
        }
        self.controller
            .write_frame(&mut self.bus, channel, data)
            .await
    }

    /// Clears display RAM for a targeted color channel using a fill byte pattern.
    pub async fn clear_frame(
        &mut self,
        channel: ColorChannel,
        pattern_byte: u8,
    ) -> Result<(), CONTROLLER::Error> {
        // Rows are byte-addressed in controller RAM, so a panel whose width is not a multiple
        // of 8 (such as the 122 px GDEM0213B74) still occupies `width.div_ceil(8)` bytes per row.
        let total_bytes = PANEL::WIDTH.div_ceil(8) as usize * PANEL::HEIGHT as usize;
        self.controller
            .write_frame_pattern(&mut self.bus, channel, pattern_byte, total_bytes)
            .await
    }

    /// Triggers display update refresh sequence.
    pub async fn refresh<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<(), CONTROLLER::Error> {
        self.controller.trigger_refresh(&mut self.bus, delay).await
    }

    /// Puts controller into sleep state.
    pub async fn sleep<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Result<(), CONTROLLER::Error> {
        self.controller.sleep(&mut self.bus, delay).await
    }
}

/// Builder pattern orchestrator for `EpdDriver`.
pub struct EpdBuilder<CONTROLLER, PANEL> {
    controller: CONTROLLER,
    _panel: PhantomData<PANEL>,
}

impl<CONTROLLER, PANEL> EpdBuilder<CONTROLLER, PANEL> {
    /// Creates a new driver builder with specified controller instance.
    pub fn new(controller: CONTROLLER) -> Self {
        Self {
            controller,
            _panel: PhantomData,
        }
    }

    /// Consumes builder and instantiates `EpdDriver` given a communication bus.
    pub fn build<BUS>(self, bus: BUS) -> EpdDriver<BUS, CONTROLLER, PANEL>
    where
        CONTROLLER: EpdController<BUS>,
        PANEL: EpdPanel,
    {
        EpdDriver::new(bus, self.controller)
    }
}

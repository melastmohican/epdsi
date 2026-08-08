//! Display driver orchestrator and builder implementation.

use core::marker::PhantomData;
use embedded_hal::delay::DelayNs;

use crate::traits::{ColorChannel, EpdController, EpdPanel};

/// Primary display driver orchestrating physical communications, controller logic, and panel dimensions.
pub struct EpdDriver<BUS, CONTROLLER, PANEL> {
    bus: BUS,
    controller: CONTROLLER,
    _panel: PhantomData<PANEL>,
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

    /// Initializes hardware reset, command registers, and panel configuration.
    pub fn init<DELAY: DelayNs>(&mut self, delay: &mut DELAY) -> Result<(), CONTROLLER::Error> {
        self.controller.init_sequence(&mut self.bus, delay)
    }

    /// Sets display RAM active window boundaries.
    pub fn set_window(
        &mut self,
        x_start: u32,
        y_start: u32,
        x_end: u32,
        y_end: u32,
    ) -> Result<(), CONTROLLER::Error> {
        self.controller
            .set_window(&mut self.bus, x_start, y_start, x_end, y_end)
    }

    /// Sets display RAM cursor position.
    pub fn set_cursor(&mut self, x: u32, y: u32) -> Result<(), CONTROLLER::Error> {
        self.controller.set_cursor(&mut self.bus, x, y)
    }

    /// Writes raw slice data into display controller RAM.
    pub fn write_frame(
        &mut self,
        channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), CONTROLLER::Error> {
        self.controller.write_frame(&mut self.bus, channel, data)
    }

    /// Clears display RAM for a targeted color channel using a fill byte pattern.
    pub fn clear_frame(
        &mut self,
        channel: ColorChannel,
        pattern_byte: u8,
    ) -> Result<(), CONTROLLER::Error> {
        let total_bytes = ((PANEL::WIDTH * PANEL::HEIGHT) / 8) as usize;
        self.controller
            .write_frame_pattern(&mut self.bus, channel, pattern_byte, total_bytes)
    }

    /// Triggers display update refresh sequence.
    pub fn refresh<DELAY: DelayNs>(&mut self, delay: &mut DELAY) -> Result<(), CONTROLLER::Error> {
        self.controller.trigger_refresh(&mut self.bus, delay)
    }

    /// Puts controller into sleep state.
    pub fn sleep<DELAY: DelayNs>(&mut self, delay: &mut DELAY) -> Result<(), CONTROLLER::Error> {
        self.controller.sleep(&mut self.bus, delay)
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

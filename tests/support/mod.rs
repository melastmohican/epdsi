//! Shared dual-mode test harness. One `RecordingSpiBus`/`TestDc`/`DummyPin`/`DummyDelay`
//! definition instead of six copy-pasted ones, and the `epd_test!` macro so each test's logic is
//! written once (async-first, `#[maybe_async_cfg::maybe]`-annotated) and runs under both
//! `cargo test` (blocking) and `cargo test --no-default-features --features graphics` (async).
#![allow(dead_code)]

use core::cell::RefCell;

#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::spi::SpiDevice;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::spi::SpiDevice;

use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait;
use embedded_hal::spi::{ErrorKind, ErrorType as SpiErrorType, Operation};

/// Picks a direct call (blocking) or `pollster::block_on` (async) for one test body function,
/// so the body itself — written once, async-first — is the only thing that differs per test.
#[macro_export]
macro_rules! epd_test {
    ($name:ident, $body:ident) => {
        #[test]
        fn $name() {
            #[cfg(feature = "blocking")]
            {
                $body();
            }
            #[cfg(not(feature = "blocking"))]
            {
                pollster::block_on($body());
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpiRecord {
    Command(u8),
    Data(Vec<u8>),
}

#[derive(Debug)]
pub struct RecordingSpiBus {
    pub records: RefCell<Vec<SpiRecord>>,
    dc_state: RefCell<bool>, // false = Low (Command), true = High (Data)
}

impl RecordingSpiBus {
    pub fn new() -> Self {
        Self {
            records: RefCell::new(Vec::new()),
            dc_state: RefCell::new(false),
        }
    }
}

impl Default for RecordingSpiBus {
    fn default() -> Self {
        Self::new()
    }
}

impl SpiErrorType for &RecordingSpiBus {
    type Error = ErrorKind;
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl SpiDevice for &RecordingSpiBus {
    async fn transaction(
        &mut self,
        _operations: &mut [Operation<'_, u8>],
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        let is_data = *self.dc_state.borrow();
        if is_data {
            self.records
                .borrow_mut()
                .push(SpiRecord::Data(buf.to_vec()));
        } else {
            for &byte in buf {
                self.records.borrow_mut().push(SpiRecord::Command(byte));
            }
        }
        Ok(())
    }
}

pub struct TestDc<'a>(pub &'a RecordingSpiBus);

impl DigitalErrorType for TestDc<'_> {
    type Error = core::convert::Infallible;
}
impl OutputPin for TestDc<'_> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        *self.0.dc_state.borrow_mut() = false;
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        *self.0.dc_state.borrow_mut() = true;
        Ok(())
    }
}

#[derive(Debug)]
pub struct DummyPin;
impl DigitalErrorType for DummyPin {
    type Error = core::convert::Infallible;
}
impl OutputPin for DummyPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl InputPin for DummyPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(true)
    }
}
#[cfg(not(feature = "blocking"))]
impl Wait for DummyPin {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// A `BUSY` pin fixed at one level, for tests that exercise a controller's busy-wait handling
/// directly (as opposed to `DummyPin`, which is never busy).
#[derive(Debug)]
pub struct FixedPin(pub bool);
impl DigitalErrorType for FixedPin {
    type Error = core::convert::Infallible;
}
impl OutputPin for FixedPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
impl InputPin for FixedPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(self.0)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(!self.0)
    }
}
#[cfg(not(feature = "blocking"))]
impl Wait for FixedPin {
    async fn wait_for_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_rising_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_falling_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    async fn wait_for_any_edge(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

#[derive(Debug)]
pub struct DummyDelay;

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl DelayNs for DummyDelay {
    async fn delay_ns(&mut self, _ns: u32) {}
    async fn delay_us(&mut self, _us: u32) {}
    async fn delay_ms(&mut self, _ms: u32) {}
}

/// Flattens the 64-byte chunks `send_data_repeated` streams into one contiguous `Vec<u8>`, so a
/// full-plane assertion can compare against one expected buffer instead of ~750 individual
/// `SpiRecord::Data` chunks.
pub fn coalesce(records: &[SpiRecord]) -> Vec<SpiRecord> {
    let mut out: Vec<SpiRecord> = Vec::new();
    for record in records {
        match (out.last_mut(), record) {
            (Some(SpiRecord::Data(prev)), SpiRecord::Data(next)) => prev.extend_from_slice(next),
            _ => out.push(record.clone()),
        }
    }
    out
}

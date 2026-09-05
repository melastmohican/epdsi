//! SPI communication bus wrapper compatible with `embedded-hal` 1.0.

use core::fmt::Debug;

// `DelayNs` and `SpiDevice` share the same name across `embedded-hal` and `embedded-hal-async` —
// only the crate differs — so a plain `cfg`'d import swap resolves every `DelayNs`/`SpiDevice`
// bound in this file to the right trait per feature, with no per-identifier macro mapping needed.
#[cfg(feature = "blocking")]
use embedded_hal::delay::DelayNs;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::delay::DelayNs;
#[cfg(feature = "blocking")]
use embedded_hal::spi::SpiDevice;
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::spi::SpiDevice;

use embedded_hal::digital::{InputPin, OutputPin};
// No async `OutputPin` exists in `embedded-hal-async` — setting a GPIO pin isn't treated as an
// async operation. `Wait` is the async-only counterpart used for edge-triggered busy polling.
#[cfg(not(feature = "blocking"))]
use embedded_hal_async::digital::Wait;

/// Errors caught before any bytes reach the bus — a caller-supplied buffer or window that
/// cannot be honoured against the panel's declared dimensions.
///
/// Bus-agnostic and non-generic so [`EpdController::Error`](crate::traits::EpdController::Error)
/// implementations can convert into it without knowing a concrete bus's pin/SPI error types —
/// see the blanket [`From`] impl on [`EpdBusError`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum ValidationError {
    /// A `write_frame` buffer is smaller than the channel requires for the panel's dimensions.
    BufferTooSmall {
        /// Minimum buffer length the channel/panel combination requires.
        required: usize,
        /// Actual length of the buffer the caller supplied.
        provided: usize,
    },
    /// A `set_window` call named coordinates outside the panel, or an inverted range.
    InvalidWindow,
}

/// Bus error wrapper categorizing errors from SPI transfers or GPIO toggling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[non_exhaustive]
pub enum EpdBusError<SPIE, DCE, RSTE, BUSYE> {
    /// Error originating from SPI transfer.
    Spi(SPIE),
    /// Error toggling Data/Command pin.
    Dc(DCE),
    /// Error toggling Reset pin.
    Reset(RSTE),
    /// Error reading Busy input pin.
    Busy(BUSYE),
    /// A `write_frame` buffer is smaller than the channel requires for the panel's dimensions.
    BufferTooSmall {
        /// Minimum buffer length the channel/panel combination requires.
        required: usize,
        /// Actual length of the buffer the caller supplied.
        provided: usize,
    },
    /// A `set_window` call named coordinates outside the panel, or an inverted range.
    InvalidWindow,
}

impl<SPIE, DCE, RSTE, BUSYE> From<ValidationError> for EpdBusError<SPIE, DCE, RSTE, BUSYE> {
    fn from(err: ValidationError) -> Self {
        match err {
            ValidationError::BufferTooSmall { required, provided } => {
                EpdBusError::BufferTooSmall { required, provided }
            }
            ValidationError::InvalidWindow => EpdBusError::InvalidWindow,
        }
    }
}

/// Alias for SPI bus operation results.
pub type SpiBusResult<SPIE, DCE, RSTE, BUSYE, T = ()> =
    Result<T, EpdBusError<SPIE, DCE, RSTE, BUSYE>>;

/// SPI Bus Wrapper holding SPI device and control GPIO pins (DC, RST, BUSY).
pub struct SpiBusWrapper<SPI, DC, RST, BUSY> {
    spi: SPI,
    dc: DC,
    rst: RST,
    busy: BUSY,
}

// Feature-agnostic accessors: none of these touch `SPI::write`/`DELAY::delay_ms`, so nothing
// here differs between the two modes. Kept in their own always-compiled block rather than the
// `maybe_async_cfg`-annotated one below, so there's no question of whether a non-`async fn` item
// mixed into that block would be left alone or misprocessed.
impl<SPI, DC, RST, BUSY> SpiBusWrapper<SPI, DC, RST, BUSY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    /// Constructs a new SPI communication bus wrapper.
    pub fn new(spi: SPI, dc: DC, rst: RST, busy: BUSY) -> Self {
        Self { spi, dc, rst, busy }
    }

    /// Reads the current level of the BUSY pin.
    ///
    /// Panels signal "still working" on this line, with the active level differing per
    /// panel — see the `busy_active_high` argument on [`Self::wait_busy`]. Exposed so
    /// callers can observe or time a refresh instead of only blocking on it, which is
    /// often the only way to tell a slow panel from a stalled one.
    pub fn busy_is_high(
        &mut self,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error, bool> {
        self.busy.is_high().map_err(EpdBusError::Busy)
    }

    /// Access inner SPI device reference.
    pub fn spi_mut(&mut self) -> &mut SPI {
        &mut self.spi
    }
}

/// Mechanical async-first block: every method here is a one-for-one match with its pre-0.2.0
/// blocking body plus `.await` on the SPI/delay calls. `maybe_async_cfg` generates the blocking
/// twin by stripping `async`/`.await`, gated on `blocking` vs. `not(blocking)` — see the crate's
/// 0.2.0 CHANGELOG entry for why `blocking` is on by default.
#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
impl<SPI, DC, RST, BUSY> SpiBusWrapper<SPI, DC, RST, BUSY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    /// Performs hardware reset sequence using RST pin and delay provider.
    pub async fn hard_reset<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        reset_duration_ms: u32,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        self.rst.set_high().map_err(EpdBusError::Reset)?;
        delay.delay_ms(5).await;
        self.rst.set_low().map_err(EpdBusError::Reset)?;
        delay.delay_ms(reset_duration_ms).await;
        self.rst.set_high().map_err(EpdBusError::Reset)?;
        delay.delay_ms(reset_duration_ms).await;
        Ok(())
    }

    /// Sends a single command byte over SPI with DC pin driven LOW.
    pub async fn send_command(
        &mut self,
        command: u8,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        self.dc.set_low().map_err(EpdBusError::Dc)?;
        self.spi.write(&[command]).await.map_err(EpdBusError::Spi)
    }

    /// Sends a slice of data bytes over SPI with DC pin driven HIGH.
    pub async fn send_data(
        &mut self,
        data: &[u8],
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        if data.is_empty() {
            return Ok(());
        }
        self.dc.set_high().map_err(EpdBusError::Dc)?;
        self.spi.write(data).await.map_err(EpdBusError::Spi)
    }

    /// Sends a command byte followed by a data slice.
    pub async fn send_command_with_data(
        &mut self,
        command: u8,
        data: &[u8],
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        self.send_command(command).await?;
        self.send_data(data).await
    }

    /// Repeatedly sends a byte `count` times with DC pin driven HIGH.
    pub async fn send_data_repeated(
        &mut self,
        byte: u8,
        count: usize,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        if count == 0 {
            return Ok(());
        }
        self.dc.set_high().map_err(EpdBusError::Dc)?;
        let chunk = [byte; 64];
        let mut remaining = count;
        while remaining > 0 {
            let write_len = remaining.min(chunk.len());
            self.spi
                .write(&chunk[..write_len])
                .await
                .map_err(EpdBusError::Spi)?;
            remaining -= write_len;
        }
        Ok(())
    }
}

/// Busy-wait methods, blocking mode: unchanged from 0.1.7 — a bounded spin/delay poll, since a
/// blocking wait has no other way to bound its own cost. See the async block below for why the
/// two modes deliberately behave differently here, not just mechanically.
#[cfg(feature = "blocking")]
impl<SPI, DC, RST, BUSY> SpiBusWrapper<SPI, DC, RST, BUSY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    /// Polls BUSY pin until display controller signals idle state.
    ///
    /// `busy_active_high`: `true` if HIGH indicates busy, `false` if LOW indicates busy.
    pub fn wait_busy(
        &mut self,
        busy_active_high: bool,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        let mut retries = 0u64;
        loop {
            let is_busy = self.busy.is_high().map_err(EpdBusError::Busy)?;
            if is_busy == busy_active_high {
                // Yield/spin briefly
                core::hint::spin_loop();
                retries += 1;
                if retries > 1_000_000_000 {
                    // Safety timeout after max iterations
                    break;
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Polls BUSY pin until display controller signals idle state, delaying between iterations.
    ///
    /// `busy_active_high`: `true` if HIGH indicates busy, `false` if LOW indicates busy.
    pub fn wait_busy_with_delay<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        busy_active_high: bool,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        let mut retries = 0u32;
        loop {
            let is_busy = self.busy.is_high().map_err(EpdBusError::Busy)?;
            if is_busy == busy_active_high {
                delay.delay_ms(1);
                retries += 1;
                if retries > 60_000 {
                    // Safety timeout after 60,000ms (60 seconds)
                    break;
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Waits for BUSY to *assert*, up to `timeout_ms`, reporting whether it was observed.
    ///
    /// Controllers do not raise BUSY the instant a command lands, and the host may still be
    /// draining its SPI FIFO when the write call returns. Polling for completion in that window
    /// reads "idle" and concludes an update finished before it started — the panel then refreshes
    /// for seconds in the background while the caller writes into RAM underneath it.
    ///
    /// A fixed settling delay does not solve this reliably: the assertion latency varies with the
    /// host, the command and the panel's state, so any constant is both too long sometimes and too
    /// short others. Waiting for the edge adapts, and the timeout bounds the cost when the panel
    /// genuinely is not responding.
    ///
    /// Returns `false` on timeout rather than erroring — a panel that never asserts is a real
    /// condition worth reporting, not a bus fault. Follow this with [`Self::wait_busy_with_delay`]
    /// to wait for the operation to finish; that call returns immediately if BUSY never asserted,
    /// so a missing panel still cannot hang.
    ///
    /// `busy_active_high`: `true` if HIGH indicates busy, `false` if LOW indicates busy.
    pub fn wait_busy_assert<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        busy_active_high: bool,
        timeout_ms: u32,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error, bool> {
        for _ in 0..timeout_ms {
            if self.busy.is_high().map_err(EpdBusError::Busy)? == busy_active_high {
                return Ok(true);
            }
            delay.delay_ms(1);
        }
        Ok(false)
    }
}

/// Busy-wait methods, async mode: edge-triggered via [`Wait`], with **no retry cap** — unlike the
/// blocking spin/delay poll, this has nothing to bound itself against, and `embedded-hal-async`'s
/// `Wait` trait offers no timeout primitive to race against (that needs an executor, e.g.
/// `embassy_time::with_timeout` wrapping the whole call). This is a deliberate behavior
/// difference between the two modes, not an oversight — matching how `weact-studio-epd`'s own
/// async `wait_until_idle` has no timeout either. `delay`/`timeout_ms` stay as parameters purely
/// so call sites (written once, shared by both modes) don't need to branch.
#[cfg(not(feature = "blocking"))]
impl<SPI, DC, RST, BUSY> SpiBusWrapper<SPI, DC, RST, BUSY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: Wait,
{
    /// Waits until display controller signals idle state.
    ///
    /// `busy_active_high`: `true` if HIGH indicates busy, `false` if LOW indicates busy.
    pub async fn wait_busy(
        &mut self,
        busy_active_high: bool,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        if busy_active_high {
            let _ = self.busy.wait_for_low().await;
        } else {
            let _ = self.busy.wait_for_high().await;
        }
        Ok(())
    }

    /// Waits until display controller signals idle state. Identical to [`Self::wait_busy`] in
    /// async mode — `delay` is unused here; it exists so blocking and async call sites match.
    ///
    /// `busy_active_high`: `true` if HIGH indicates busy, `false` if LOW indicates busy.
    pub async fn wait_busy_with_delay<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        busy_active_high: bool,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        let _ = delay;
        self.wait_busy(busy_active_high).await
    }

    /// Waits for BUSY to *assert*, then always reports `true` — `timeout_ms`/`delay` are unused
    /// in async mode; see the block doc for why a bounded wait isn't available here.
    ///
    /// `busy_active_high`: `true` if HIGH indicates busy, `false` if LOW indicates busy.
    pub async fn wait_busy_assert<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        busy_active_high: bool,
        timeout_ms: u32,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error, bool> {
        let _ = (delay, timeout_ms);
        if busy_active_high {
            let _ = self.busy.wait_for_high().await;
        } else {
            let _ = self.busy.wait_for_low().await;
        }
        Ok(true)
    }
}

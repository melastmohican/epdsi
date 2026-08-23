//! SPI communication bus wrapper compatible with `embedded-hal` 1.0.

use core::fmt::Debug;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

/// Bus error wrapper categorizing errors from SPI transfers or GPIO toggling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum EpdBusError<SPIE, DCE, RSTE, BUSYE> {
    /// Error originating from SPI transfer.
    Spi(SPIE),
    /// Error toggling Data/Command pin.
    Dc(DCE),
    /// Error toggling Reset pin.
    Reset(RSTE),
    /// Error reading Busy input pin.
    Busy(BUSYE),
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

    /// Performs hardware reset sequence using RST pin and delay provider.
    pub fn hard_reset<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        reset_duration_ms: u32,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        self.rst.set_high().map_err(EpdBusError::Reset)?;
        delay.delay_ms(5);
        self.rst.set_low().map_err(EpdBusError::Reset)?;
        delay.delay_ms(reset_duration_ms);
        self.rst.set_high().map_err(EpdBusError::Reset)?;
        delay.delay_ms(reset_duration_ms);
        Ok(())
    }

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

    /// Sends a single command byte over SPI with DC pin driven LOW.
    pub fn send_command(
        &mut self,
        command: u8,
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        self.dc.set_low().map_err(EpdBusError::Dc)?;
        self.spi.write(&[command]).map_err(EpdBusError::Spi)
    }

    /// Sends a slice of data bytes over SPI with DC pin driven HIGH.
    pub fn send_data(
        &mut self,
        data: &[u8],
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        if data.is_empty() {
            return Ok(());
        }
        self.dc.set_high().map_err(EpdBusError::Dc)?;
        self.spi.write(data).map_err(EpdBusError::Spi)
    }

    /// Sends a command byte followed by a data slice.
    pub fn send_command_with_data(
        &mut self,
        command: u8,
        data: &[u8],
    ) -> SpiBusResult<SPI::Error, DC::Error, RST::Error, BUSY::Error> {
        self.send_command(command)?;
        self.send_data(data)
    }

    /// Repeatedly sends a byte `count` times with DC pin driven HIGH.
    pub fn send_data_repeated(
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
                .map_err(EpdBusError::Spi)?;
            remaining -= write_len;
        }
        Ok(())
    }
}

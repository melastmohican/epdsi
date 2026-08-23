//! Bit-banged "3-wire" half-duplex SPI bus for the Pervasive Displays BWRY COG family's OTP
//! register read handshake.
//!
//! The BWRY reference driver (`Pervasive_BWRY_Small`'s `hV_HAL_SPI3_*` routines) does **not** use
//! the hardware SPI peripheral for its OTP/chip-ID read: it bit-bangs a clock pin and a single
//! bidirectional data pin (by default the same physical wires as SCK/MOSI), switching the data
//! pin's direction between push-pull output (write) and floating input (read) per byte, with the
//! panel driving its response back on that same wire. `SpiBusWrapper`'s `SpiDevice`-based model
//! cannot express this (a `SpiDevice`'s CS handling is opaque, and full-duplex reads assume a
//! separate MISO line the panel never drives during this handshake) — hence this dedicated type.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};

/// A GPIO pin whose direction can be switched at runtime between push-pull output and floating
/// input, required for the bit-banged bidirectional DATA line used by [`Spi3Bus`].
pub trait DynamicPin {
    /// Error type for pin operations.
    type Error: core::fmt::Debug;

    /// Reconfigures the pin as a push-pull output.
    fn set_as_output(&mut self) -> Result<(), Self::Error>;

    /// Reconfigures the pin as a floating input.
    fn set_as_input(&mut self) -> Result<(), Self::Error>;

    /// Drives the pin high. Only valid while configured as an output.
    fn set_high(&mut self) -> Result<(), Self::Error>;

    /// Drives the pin low. Only valid while configured as an output.
    fn set_low(&mut self) -> Result<(), Self::Error>;

    /// Reads the pin level. Only valid while configured as an input.
    fn is_high(&mut self) -> Result<bool, Self::Error>;
}

/// Error wrapper categorizing errors from the individual GPIO pins driving [`Spi3Bus`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Spi3BusError<CSE, SCKE, DATAE, DCE, RSTE, BUSYE> {
    /// Error toggling the Chip Select pin.
    Cs(CSE),
    /// Error toggling the bit-banged Clock pin.
    Sck(SCKE),
    /// Error toggling or reading the bidirectional Data pin.
    Data(DATAE),
    /// Error toggling the Data/Command pin.
    Dc(DCE),
    /// Error toggling the Reset pin.
    Reset(RSTE),
    /// Error reading the Busy input pin.
    Busy(BUSYE),
}

/// Alias for [`Spi3Bus`] operation results.
pub type Spi3BusResult<CSE, SCKE, DATAE, DCE, RSTE, BUSYE, T = ()> =
    Result<T, Spi3BusError<CSE, SCKE, DATAE, DCE, RSTE, BUSYE>>;

/// Bit-banged 3-wire (SCK + bidirectional DATA) bus wrapper, plus the CS/DC/RST/BUSY GPIO pins
/// needed to frame each byte exactly like the reference driver's `hV_HAL_SPI3_write`/`_read`.
pub struct Spi3Bus<CS, SCK, DATA, DC, RST, BUSY> {
    cs: CS,
    sck: SCK,
    data: DATA,
    dc: DC,
    rst: RST,
    busy: BUSY,
}

#[allow(clippy::type_complexity)]
impl<CS, SCK, DATA, DC, RST, BUSY> Spi3Bus<CS, SCK, DATA, DC, RST, BUSY>
where
    CS: OutputPin,
    SCK: OutputPin,
    DATA: DynamicPin,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    /// Constructs a new bit-banged 3-wire bus wrapper.
    pub fn new(cs: CS, sck: SCK, data: DATA, dc: DC, rst: RST, busy: BUSY) -> Self {
        Self {
            cs,
            sck,
            data,
            dc,
            rst,
            busy,
        }
    }

    /// Consumes the bus, returning its individual pins — typically used to reclaim CS/DC/RST/BUSY
    /// for the normal `SpiBusWrapper`-based bus afterward, and to reconfigure SCK/DATA into the
    /// hardware SPI peripheral's function once the bit-banged OTP handshake is done.
    pub fn release(self) -> (CS, SCK, DATA, DC, RST, BUSY) {
        (self.cs, self.sck, self.data, self.dc, self.rst, self.busy)
    }

    /// Performs the hardware reset sequence matching the reference driver's `COG_reset`/`b_reset`
    /// timing (delay 20ms, RST high, delay 10ms, RST low, delay 20ms, RST high, delay 10ms), then
    /// waits for the busy pin to signal idle.
    pub fn reset<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Spi3BusResult<CS::Error, SCK::Error, DATA::Error, DC::Error, RST::Error, BUSY::Error> {
        delay.delay_ms(20);
        self.rst.set_high().map_err(Spi3BusError::Reset)?;
        delay.delay_ms(10);
        self.rst.set_low().map_err(Spi3BusError::Reset)?;
        delay.delay_ms(20);
        self.rst.set_high().map_err(Spi3BusError::Reset)?;
        delay.delay_ms(10);
        self.cs.set_high().map_err(Spi3BusError::Cs)?;
        delay.delay_ms(10);
        self.wait_busy(delay)
    }

    /// Polls the BUSY pin until idle (active-low: busy while LOW), delaying between iterations.
    pub fn wait_busy<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Spi3BusResult<CS::Error, SCK::Error, DATA::Error, DC::Error, RST::Error, BUSY::Error> {
        let mut retries = 0u32;
        loop {
            let is_busy = !self.busy.is_high().map_err(Spi3BusError::Busy)?;
            if is_busy {
                delay.delay_ms(1);
                retries += 1;
                if retries > 1_500 {
                    break;
                }
            } else {
                break;
            }
        }
        Ok(())
    }

    /// Bit-bangs a single byte onto the DATA line, MSB first, toggling SCK once per bit.
    fn write_byte<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        value: u8,
    ) -> Spi3BusResult<CS::Error, SCK::Error, DATA::Error, DC::Error, RST::Error, BUSY::Error> {
        self.data.set_as_output().map_err(Spi3BusError::Data)?;
        for i in 0..8 {
            if value & (1 << (7 - i)) != 0 {
                self.data.set_high().map_err(Spi3BusError::Data)?;
            } else {
                self.data.set_low().map_err(Spi3BusError::Data)?;
            }
            delay.delay_us(1);
            self.sck.set_high().map_err(Spi3BusError::Sck)?;
            delay.delay_us(1);
            self.sck.set_low().map_err(Spi3BusError::Sck)?;
            delay.delay_us(1);
        }
        Ok(())
    }

    /// Bit-bangs a single byte in from the DATA line, MSB first, toggling SCK once per bit.
    fn read_byte<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Spi3BusResult<CS::Error, SCK::Error, DATA::Error, DC::Error, RST::Error, BUSY::Error, u8>
    {
        self.data.set_as_input().map_err(Spi3BusError::Data)?;
        let mut value = 0u8;
        for i in 0..8 {
            self.sck.set_high().map_err(Spi3BusError::Sck)?;
            delay.delay_us(1);
            if self.data.is_high().map_err(Spi3BusError::Data)? {
                value |= 1 << (7 - i);
            }
            self.sck.set_low().map_err(Spi3BusError::Sck)?;
            delay.delay_us(1);
        }
        Ok(value)
    }

    /// Sends a single command byte (DC low), bracketed by its own CS select/unselect pulse.
    pub fn write_cmd<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        byte: u8,
    ) -> Spi3BusResult<CS::Error, SCK::Error, DATA::Error, DC::Error, RST::Error, BUSY::Error> {
        self.dc.set_low().map_err(Spi3BusError::Dc)?;
        self.cs.set_low().map_err(Spi3BusError::Cs)?;
        self.write_byte(delay, byte)?;
        self.cs.set_high().map_err(Spi3BusError::Cs)
    }

    /// Sends a single data byte (DC high), bracketed by its own CS select/unselect pulse.
    pub fn write_data<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
        byte: u8,
    ) -> Spi3BusResult<CS::Error, SCK::Error, DATA::Error, DC::Error, RST::Error, BUSY::Error> {
        self.dc.set_high().map_err(Spi3BusError::Dc)?;
        self.cs.set_low().map_err(Spi3BusError::Cs)?;
        self.write_byte(delay, byte)?;
        self.cs.set_high().map_err(Spi3BusError::Cs)
    }

    /// Reads a single data byte (DC high), bracketed by its own CS select/unselect pulse.
    pub fn read_data_byte<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Spi3BusResult<CS::Error, SCK::Error, DATA::Error, DC::Error, RST::Error, BUSY::Error, u8>
    {
        self.dc.set_high().map_err(Spi3BusError::Dc)?;
        self.cs.set_low().map_err(Spi3BusError::Cs)?;
        let value = self.read_byte(delay)?;
        self.cs.set_high().map_err(Spi3BusError::Cs)?;
        Ok(value)
    }

    /// Reads a single byte (DC left as previously set), bracketed by its own CS select/unselect
    /// pulse, without touching DC — used for the bulk OTP-populate loop where DC is already HIGH
    /// and the reference driver never re-sets it per byte.
    pub fn read_byte_no_dc<DELAY: DelayNs>(
        &mut self,
        delay: &mut DELAY,
    ) -> Spi3BusResult<CS::Error, SCK::Error, DATA::Error, DC::Error, RST::Error, BUSY::Error, u8>
    {
        self.cs.set_low().map_err(Spi3BusError::Cs)?;
        let value = self.read_byte(delay)?;
        self.cs.set_high().map_err(Spi3BusError::Cs)?;
        Ok(value)
    }
}

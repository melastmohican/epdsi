//! Pervasive Displays BWRY (Black/White/Red/Yellow, Spectra-4) E-Paper Display Controller implementation.
//!
//! Unlike [`crate::controllers::pervasive_bw::PervasiveBwController`] (DriverC/DriverF, BWR family),
//! the BWRY COG family sources nearly all of its init-time register values (PSR, booster, PLL, CDI/VCOM,
//! resolution) from the panel's on-chip OTP memory at runtime via a dedicated read handshake, rather than
//! from static hardcoded bytes. This controller implements that OTP read protocol.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::traits::{ColorChannel, EpdController};

/// Pervasive Displays BWRY Command Register Definitions
pub mod cmd {
    /// Panel Setting Register (PSR)
    pub const PSR: u8 = 0x00;
    /// Power Off command
    pub const POWER_OFF: u8 = 0x02;
    /// Power On command
    pub const POWER_ON: u8 = 0x04;
    /// Write image data (single packed 2bpp BWRY plane)
    pub const WRITE_DATA: u8 = 0x10;
    /// Display Refresh command (DRF)
    pub const DISPLAY_REFRESH: u8 = 0x12;
    /// VCOM and Data Interval Setting (CDI)
    pub const CDI: u8 = 0x50;
    /// Active/state select register
    pub const ACTIVE_STATE: u8 = 0xE0;
    /// Input Temperature value selection (BWRY uses 0xE6, not the BWR family's 0xE5)
    pub const INPUT_TEMP: u8 = 0xE6;
    /// OTP read chip-ID command
    pub const READ_CHIP_ID: u8 = 0x70;
}

/// Pervasive BWRY COG driver IC variant family.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PervasiveBwryVariant {
    /// Driver 6 (e.g. `E2154QS0F1` / `EPD_152_QS_06`). 48-byte OTP read, chip ID `0x4801`.
    #[default]
    Driver6,
    /// Driver A (e.g. `E2417QS0A3` / `EPD_417_QS_0A`). 112-byte OTP read (with bank-2 fallback), chip ID `0x0605`.
    DriverA,
}

impl PervasiveBwryVariant {
    /// Expected OTP chip-ID handshake value for this variant.
    fn expected_chip_id(self) -> u16 {
        match self {
            Self::Driver6 => 0x4801,
            Self::DriverA => 0x0605,
        }
    }

    /// Number of OTP bytes to read for this variant.
    fn otp_len(self) -> usize {
        match self {
            Self::Driver6 => 48,
            Self::DriverA => 112,
        }
    }
}

/// Error type for [`PervasiveBwryController`], extending bus errors with OTP handshake failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PervasiveBwryError<SPIE, DCE, RSTE, BUSYE> {
    /// Underlying SPI/GPIO bus error.
    Bus(EpdBusError<SPIE, DCE, RSTE, BUSYE>),
    /// OTP chip-ID handshake returned an unexpected value.
    UnexpectedChipId(u16),
    /// OTP bank-start marker (`0xA5`) was not found at either the primary or bank-2 fallback offset.
    InvalidOtpMarker,
}

impl<SPIE, DCE, RSTE, BUSYE> From<EpdBusError<SPIE, DCE, RSTE, BUSYE>>
    for PervasiveBwryError<SPIE, DCE, RSTE, BUSYE>
{
    fn from(e: EpdBusError<SPIE, DCE, RSTE, BUSYE>) -> Self {
        Self::Bus(e)
    }
}

/// Pervasive Displays BWRY (Black/White/Red/Yellow) COG Controller IC driver implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PervasiveBwryController {
    width: u32,
    height: u32,
    temperature_c: i8,
    variant: PervasiveBwryVariant,
    /// OTP-derived register data, sized for DriverA's larger case (112 bytes); Driver6 only populates `[0..48]`.
    otp_data: [u8; 112],
}

impl PervasiveBwryController {
    /// Creates a new Pervasive BWRY controller instance with target resolution.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            temperature_c: 25,
            variant: PervasiveBwryVariant::default(),
            otp_data: [0u8; 112],
        }
    }

    /// Sets operating ambient temperature in Celsius (builder method).
    pub fn with_temperature(mut self, temperature_c: i8) -> Self {
        self.temperature_c = temperature_c;
        self
    }

    /// Sets ambient temperature in Celsius.
    pub fn set_temperature(&mut self, temperature_c: i8) {
        self.temperature_c = temperature_c;
    }

    /// Returns current operating ambient temperature in Celsius.
    pub fn temperature(&self) -> i8 {
        self.temperature_c
    }

    /// Sets driver IC variant (builder method).
    pub fn with_variant(mut self, variant: PervasiveBwryVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Sets driver IC variant.
    pub fn set_variant(&mut self, variant: PervasiveBwryVariant) {
        self.variant = variant;
    }

    /// Returns current driver IC variant.
    pub fn variant(&self) -> PervasiveBwryVariant {
        self.variant
    }

    /// Performs the OTP chip-ID handshake and unlock+read sequence for the configured variant,
    /// populating `self.otp_data`.
    #[allow(clippy::type_complexity)]
    fn read_otp<SPI, DC, RST, BUSY, DELAY>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), PervasiveBwryError<SPI::Error, DC::Error, RST::Error, BUSY::Error>>
    where
        SPI: SpiDevice,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
        DELAY: DelayNs,
    {
        // Chip-ID handshake (shared preamble for both variants)
        bus.send_command(cmd::READ_CHIP_ID)?;
        let mut id_buf = [0u8; 2];
        bus.read_data(&mut id_buf)?;
        let raw = u16::from_be_bytes(id_buf);
        let id = if raw == 0x8302 { 0x0302 } else { raw };
        if id != self.variant.expected_chip_id() {
            return Err(PervasiveBwryError::UnexpectedChipId(id));
        }

        let otp_len = self.variant.otp_len();

        match self.variant {
            PervasiveBwryVariant::Driver6 => {
                bus.send_command_with_data(0xf0, &[0x0b])?;
                bus.send_command(0x90)?;
                bus.wait_busy_with_delay(delay, false)?;
                bus.send_command_with_data(0xa2, &[0x33])?;
                bus.send_command(0xa0)?;
                bus.wait_busy_with_delay(delay, false)?;
                bus.send_command_with_data(0xf6, &[0x2d, 0x80])?;
                bus.send_command(0x92)?;
                delay.delay_ms(10);
                bus.discard_read_bytes(1)?;
                bus.read_data(&mut self.otp_data[0..1])?;
                bus.read_data(&mut self.otp_data[1..otp_len])?;
            }
            PervasiveBwryVariant::DriverA => {
                bus.send_command_with_data(0xa2, &[0x00, 0x15, 0x00])?;
                bus.send_command(0xa0)?;
                bus.send_command(0x92)?;
                bus.discard_read_bytes(1)?;
                bus.read_data(&mut self.otp_data[0..1])?;
                if self.otp_data[0] != 0xa5 {
                    // Bank-2 fallback: 0x70 marks the bank boundary; skip past bank 1 and re-check.
                    bus.discard_read_bytes(0x70 - 1)?;
                    bus.read_data(&mut self.otp_data[0..1])?;
                    if self.otp_data[0] != 0xa5 {
                        return Err(PervasiveBwryError::InvalidOtpMarker);
                    }
                }
                bus.read_data(&mut self.otp_data[1..otp_len])?;
            }
        }

        Ok(())
    }
}

impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>>
    for PervasiveBwryController
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    type Error = PervasiveBwryError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;

    fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // Hardware reset sequence (both Driver6 and DriverA use the same reset timing branch)
        bus.hard_reset(delay, 20)?;

        // Pervasive BWRY busy pin is active-low (busy when LOW)
        bus.wait_busy_with_delay(delay, false)?;

        self.read_otp(bus, delay)?;

        // Common preamble
        bus.send_command_with_data(cmd::ACTIVE_STATE, &[0x02])?;
        bus.send_command_with_data(cmd::INPUT_TEMP, &[self.temperature_c as u8])?;

        match self.variant {
            PervasiveBwryVariant::Driver6 => {
                bus.send_command(0xa5)?;
                bus.wait_busy_with_delay(delay, false)?;
                bus.send_command_with_data(0x01, &self.otp_data[16..18])?;
                bus.send_command_with_data(cmd::PSR, &self.otp_data[18..20])?;
                bus.wait_busy_with_delay(delay, false)?;
                bus.send_command_with_data(0x61, &self.otp_data[20..24])?;
                bus.wait_busy_with_delay(delay, false)?;
                bus.send_command_with_data(0x06, &self.otp_data[24..28])?;
                bus.send_command_with_data(0x03, &self.otp_data[30..31])?;
                bus.send_command_with_data(0xe7, &self.otp_data[33..34])?;
                bus.send_command_with_data(0x65, &self.otp_data[34..38])?;
                bus.send_command_with_data(0x30, &self.otp_data[38..39])?;
                bus.send_command_with_data(cmd::CDI, &self.otp_data[39..40])?;
                bus.send_command_with_data(0x60, &self.otp_data[40..42])?;
                bus.send_command_with_data(0xe3, &self.otp_data[42..43])?;
                bus.send_command_with_data(0x62, &self.otp_data[43..45])?;
                bus.send_command_with_data(0xe9, &[0x01])?;
            }
            PervasiveBwryVariant::DriverA => {
                bus.send_command_with_data(0x01, &self.otp_data[16..17])?;
                bus.send_command_with_data(cmd::PSR, &self.otp_data[17..19])?;
                bus.send_command_with_data(0x03, &self.otp_data[30..33])?;
                bus.send_command_with_data(0x06, &self.otp_data[23..26])?;
                bus.send_command_with_data(cmd::CDI, &self.otp_data[39..40])?;
                bus.send_command_with_data(0x60, &self.otp_data[40..42])?;
                bus.send_command_with_data(0x61, &self.otp_data[19..23])?;
                bus.send_command_with_data(0xe3, &self.otp_data[42..43])?;
                bus.send_command_with_data(0xe7, &self.otp_data[33..34])?;
                bus.send_command_with_data(0x65, &self.otp_data[34..38])?;
                bus.send_command_with_data(0x30, &self.otp_data[38..39])?;
                bus.send_command_with_data(0xe9, &[0x01])?;
                bus.send_command(cmd::POWER_ON)?;
                bus.wait_busy_with_delay(delay, false)?;
            }
        }

        Ok(())
    }

    fn set_window(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x_start: u32,
        _y_start: u32,
        _x_end: u32,
        _y_end: u32,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn set_cursor(
        &mut self,
        _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _x: u32,
        _y: u32,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write_frame(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        data: &[u8],
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::WRITE_DATA, data)?;
        Ok(())
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::WRITE_DATA)?;
        bus.send_data_repeated(byte, count)?;
        Ok(())
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        if self.variant == PervasiveBwryVariant::Driver6 {
            bus.send_command(cmd::POWER_ON)?;
            bus.wait_busy_with_delay(delay, false)?;
        }
        bus.send_command_with_data(cmd::DISPLAY_REFRESH, &[0x00])?;
        bus.wait_busy_with_delay(delay, false)?;
        Ok(())
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::POWER_OFF, &[0x00])?;
        bus.wait_busy_with_delay(delay, false)?;
        match self.variant {
            PervasiveBwryVariant::Driver6 => {
                bus.send_command_with_data(0x07, &[0xa5])?;
                delay.delay_ms(50);
            }
            PervasiveBwryVariant::DriverA => {
                bus.send_command_with_data(cmd::PSR, &self.otp_data[26..28])?;
                delay.delay_ms(100);
            }
        }
        Ok(())
    }
}

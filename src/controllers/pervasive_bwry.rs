//! Pervasive Displays BWRY (Black/White/Red/Yellow, Spectra-4) E-Paper Display Controller implementation.
//!
//! Unlike [`crate::controllers::pervasive_bw::PervasiveBwController`] (DriverC/DriverF, BWR family),
//! the BWRY COG family sources nearly all of its init-time register values (PSR, booster, PLL, CDI/VCOM,
//! resolution) from the panel's on-chip OTP memory. Reading that OTP data requires the panel's
//! bit-banged 3-wire handshake (see [`crate::bus3::Spi3Bus`]) — [`PervasiveBwryController::read_otp`]
//! must be called once, using raw GPIO pins (not the hardware SPI peripheral), before
//! [`crate::traits::EpdController::init_sequence`] is invoked through the normal `SpiBusWrapper`.

use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::spi::SpiDevice;

use crate::bus::{EpdBusError, SpiBusWrapper};
use crate::bus3::{DynamicPin, Spi3Bus, Spi3BusError};
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
    /// Driver F (e.g. `E2154QS0F1` / `EPD_154_QS_0F`, shared with `213_QS_0F`/`266_QS_0F`).
    /// 48-byte OTP read, chip ID `0x0302` (raw `0x8302` is normalized to `0x0302`).
    #[default]
    DriverF,
    /// Driver A (e.g. `E2417QS0A3` / `EPD_417_QS_0A`). 112-byte OTP read (with bank-2 fallback), chip ID `0x0605`.
    DriverA,
}

impl PervasiveBwryVariant {
    /// Expected OTP chip-ID handshake value for this variant.
    fn expected_chip_id(self) -> u16 {
        match self {
            Self::DriverF => 0x0302,
            Self::DriverA => 0x0605,
        }
    }

    /// Number of OTP bytes to read for this variant.
    fn otp_len(self) -> usize {
        match self {
            Self::DriverF => 48,
            Self::DriverA => 112,
        }
    }
}

/// Error type for [`PervasiveBwryController::read_otp`], extending bit-banged bus errors with OTP
/// handshake failures.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PervasiveBwryOtpError<CSE, SCKE, DATAE, DCE, RSTE, BUSYE> {
    /// Underlying bit-banged bus/GPIO error.
    Bus(Spi3BusError<CSE, SCKE, DATAE, DCE, RSTE, BUSYE>),
    /// OTP chip-ID handshake returned an unexpected value.
    UnexpectedChipId(u16),
    /// OTP bank-start marker (`0xA5`) was not found at either the primary or bank-2 fallback offset.
    InvalidOtpMarker,
}

impl<CSE, SCKE, DATAE, DCE, RSTE, BUSYE> From<Spi3BusError<CSE, SCKE, DATAE, DCE, RSTE, BUSYE>>
    for PervasiveBwryOtpError<CSE, SCKE, DATAE, DCE, RSTE, BUSYE>
{
    fn from(e: Spi3BusError<CSE, SCKE, DATAE, DCE, RSTE, BUSYE>) -> Self {
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
    /// OTP-derived register data, sized for DriverA's larger case (112 bytes); DriverF only populates `[0..48]`.
    /// Populated by [`Self::read_otp`], which must be called before `init_sequence`.
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

    /// Performs the panel reset and OTP chip-ID/register read handshake over the bit-banged
    /// 3-wire bus, populating the register data later consumed by `init_sequence`. Must be called
    /// once, using raw GPIO pins (not the hardware SPI peripheral / `SpiBusWrapper`), before
    /// building the driver's normal `SpiBusWrapper`-based bus and calling `EpdDriver::init`.
    #[allow(clippy::type_complexity)]
    pub fn read_otp<CS, SCK, DATA, DC, RST, BUSY, DELAY>(
        &mut self,
        bus: &mut Spi3Bus<CS, SCK, DATA, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<
        (),
        PervasiveBwryOtpError<
            CS::Error,
            SCK::Error,
            DATA::Error,
            DC::Error,
            RST::Error,
            BUSY::Error,
        >,
    >
    where
        CS: OutputPin,
        SCK: OutputPin,
        DATA: DynamicPin,
        DC: OutputPin,
        RST: OutputPin,
        BUSY: InputPin,
        DELAY: DelayNs,
    {
        bus.reset(delay)?;

        // Chip-ID handshake (shared preamble for both variants)
        bus.write_cmd(delay, cmd::READ_CHIP_ID)?;
        delay.delay_ms(8);
        let hi = bus.read_data_byte(delay)?;
        let lo = bus.read_data_byte(delay)?;
        let raw = ((hi as u16) << 8) | (lo as u16);
        let id = if raw == 0x8302 { 0x0302 } else { raw };
        if id != self.variant.expected_chip_id() {
            return Err(PervasiveBwryOtpError::UnexpectedChipId(id));
        }

        let otp_len = self.variant.otp_len();

        match self.variant {
            PervasiveBwryVariant::DriverF => {
                bus.write_cmd(delay, 0xa4)?;
                bus.write_data(delay, 0x15)?;
                bus.write_data(delay, 0x00)?;
                bus.write_data(delay, 0x01)?;
                bus.wait_busy(delay)?;
                bus.write_cmd(delay, 0xa1)?;
                let _dummy = bus.read_data_byte(delay)?;
                self.otp_data[0] = bus.read_byte_no_dc(delay)?;
                if self.otp_data[0] != 0xa5 {
                    return Err(PervasiveBwryOtpError::InvalidOtpMarker);
                }
            }
            PervasiveBwryVariant::DriverA => {
                bus.write_cmd(delay, 0xa2)?;
                bus.write_data(delay, 0x00)?;
                bus.write_data(delay, 0x15)?;
                bus.write_data(delay, 0x00)?;
                bus.write_cmd(delay, 0xa0)?;
                bus.write_cmd(delay, 0x92)?;
                let _dummy = bus.read_data_byte(delay)?;
                self.otp_data[0] = bus.read_byte_no_dc(delay)?;
                if self.otp_data[0] != 0xa5 {
                    // Bank-2 fallback: discard 111 bytes, then re-check the marker.
                    for _ in 1..0x70 {
                        bus.read_byte_no_dc(delay)?;
                    }
                    self.otp_data[0] = bus.read_byte_no_dc(delay)?;
                    if self.otp_data[0] != 0xa5 {
                        return Err(PervasiveBwryOtpError::InvalidOtpMarker);
                    }
                }
            }
        }

        for i in 1..otp_len {
            self.otp_data[i] = bus.read_byte_no_dc(delay)?;
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
    type Error = EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;

    fn init_sequence<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        // Hardware reset sequence (matches the reference's per-`updateNormal()`-call COG_reset();
        // `read_otp` already performed the initial reset once as part of `begin()`).
        bus.hard_reset(delay, 20)?;

        // Pervasive BWRY busy pin is active-low (busy when LOW)
        bus.wait_busy_with_delay(delay, false)?;

        // Common preamble
        bus.send_command_with_data(cmd::ACTIVE_STATE, &[0x02])?;
        bus.send_command_with_data(cmd::INPUT_TEMP, &[self.temperature_c as u8])?;

        match self.variant {
            PervasiveBwryVariant::DriverF => {
                bus.send_command(0xa5)?;
                bus.wait_busy_with_delay(delay, false)?;
                bus.send_command_with_data(0x01, &self.otp_data[16..17])?;
                bus.send_command_with_data(cmd::PSR, &self.otp_data[17..19])?;
                bus.send_command_with_data(0x03, &self.otp_data[30..33])?;
                bus.send_command_with_data(0x06, &self.otp_data[23..30])?;
                bus.send_command_with_data(cmd::CDI, &self.otp_data[39..40])?;
                bus.send_command_with_data(0x60, &self.otp_data[40..42])?;
                bus.send_command_with_data(0x61, &self.otp_data[19..23])?;
                bus.send_command_with_data(0xe7, &self.otp_data[33..34])?;
                bus.send_command_with_data(0xe3, &self.otp_data[42..43])?;
                bus.send_command_with_data(0x4d, &self.otp_data[43..44])?;
                bus.send_command_with_data(0xb4, &self.otp_data[44..45])?;
                bus.send_command_with_data(0xb5, &self.otp_data[45..46])?;
                bus.send_command_with_data(0xe9, &[0x01])?;
                bus.send_command_with_data(0x30, &[0x08])?; // PLL (fixed, not OTP-derived for this variant)
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
        bus.send_command_with_data(cmd::WRITE_DATA, data)
    }

    fn write_frame_pattern(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        _channel: ColorChannel,
        byte: u8,
        count: usize,
    ) -> Result<(), Self::Error> {
        bus.send_command(cmd::WRITE_DATA)?;
        bus.send_data_repeated(byte, count)
    }

    fn trigger_refresh<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        if self.variant == PervasiveBwryVariant::DriverF {
            bus.send_command(cmd::POWER_ON)?;
            bus.wait_busy_with_delay(delay, false)?;
        }
        bus.send_command_with_data(cmd::DISPLAY_REFRESH, &[0x00])?;
        bus.wait_busy_with_delay(delay, false)
    }

    fn sleep<DELAY: DelayNs>(
        &mut self,
        bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
        delay: &mut DELAY,
    ) -> Result<(), Self::Error> {
        bus.send_command_with_data(cmd::POWER_OFF, &[0x00])?;
        bus.wait_busy_with_delay(delay, false)?;
        // DriverF has no additional shutdown command (falls to the reference's `default:` case);
        // DriverA re-sends the PSR from OTP-derived data before the long power-down delay.
        if self.variant == PervasiveBwryVariant::DriverA {
            bus.send_command_with_data(cmd::PSR, &self.otp_data[26..28])?;
            delay.delay_ms(100);
        }
        Ok(())
    }
}

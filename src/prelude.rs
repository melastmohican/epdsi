//! Prelude re-exporting common traits, drivers, controllers, and helpers.

pub use crate::bus::{EpdBusError, SpiBusWrapper};
pub use crate::bus3::{DynamicPin, Spi3Bus, Spi3BusError};
pub use crate::controllers::{
    Ed2208Controller, Jd79661Controller, PervasiveBwController, PervasiveBwryController,
    PervasiveBwryOtpError, PervasiveBwryVariant, PervasiveDriverVariant, PervasiveRefreshMode,
    Ssd1677Controller, Ssd1677RefreshMode, Ssd1680Controller, Ssd1680RefreshMode,
    Ssd1681Controller, Ssd1681RefreshMode, Ssd168xController, Ssd168xRefreshMode, Ssd168xVariant,
    Uc8253Controller, Uc8253RefreshMode, Uc8253Variant,
};
pub use crate::driver::{EpdBuilder, EpdDriver};
pub use crate::graphics::buffer::{DisplayRotation, PageBuffer};
pub use crate::graphics::paged::render_paged;
pub use crate::panels::{
    GxEPD2_213_B74, GxEPD2_370_GDEY037T03, GxEPD2_730c_GDEP073E01, E2154QS0F1, E2266KS0C1,
    E2290KS0F1, E2417QS0A3, EPD_154_QS_0F, EPD_266_KS_0C, EPD_290_KS_0F, EPD_417_QS_0A,
    GDEM0154Z90, GDEM0213B74, GDEP073E01, GDEQ0426T82, GDEY0213F51, GDEY037T03, SE0352N14TNGA0,
    ZJY122250_0213AJH_E5,
};
pub use crate::traits::{ColorChannel, ColorMode, EpdController, EpdPanel, SevenColor};

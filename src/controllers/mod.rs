//! Controller IC module for various EPD display drivers.

pub mod ed2208;
pub mod jd79661;
pub mod pervasive_bw;
pub mod pervasive_bwry;
pub mod ssd1677;
pub mod ssd168x;
pub mod uc8253;

pub use ed2208::Ed2208Controller;
pub use jd79661::Jd79661Controller;
pub use pervasive_bw::{PervasiveBwController, PervasiveDriverVariant, PervasiveRefreshMode};
pub use pervasive_bwry::{PervasiveBwryController, PervasiveBwryOtpError, PervasiveBwryVariant};
pub use ssd1677::{Ssd1677Controller, Ssd1677RefreshMode};
pub use ssd168x::{
    Ssd1680Controller, Ssd1680RefreshMode, Ssd1681Controller, Ssd1681RefreshMode,
    Ssd168xController, Ssd168xRefreshMode, Ssd168xVariant,
};
pub use uc8253::{Uc8253Controller, Uc8253RefreshMode, Uc8253Variant};

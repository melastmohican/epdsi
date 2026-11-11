//! Controller IC module for various EPD display drivers.

pub mod ed2208;
pub mod ist7163;
pub mod jd7966x;
pub mod pervasive_bw;
pub mod pervasive_bwry;
pub mod ssd1677;
pub mod ssd168x;
pub mod ssd2681;
pub mod uc8151d;
pub mod uc8175;
pub mod uc8253;

pub use ed2208::{Ed2208Controller, Ed2208RefreshSpeed};
pub use ist7163::Ist7163Controller;
pub use jd7966x::{
    Jd79660Controller, Jd79661Controller, Jd7966xController, Jd7966xRefreshMode, Jd7966xVariant,
    Jd79676Controller,
};
pub use pervasive_bw::{PervasiveBwController, PervasiveDriverVariant, PervasiveRefreshMode};
pub use pervasive_bwry::{PervasiveBwryController, PervasiveBwryOtpError, PervasiveBwryVariant};
pub use ssd1677::{Ssd1677Controller, Ssd1677RefreshMode};
pub use ssd168x::{
    Ssd1680Controller, Ssd1680RefreshMode, Ssd1681Controller, Ssd1681RefreshMode,
    Ssd168xController, Ssd168xRefreshMode, Ssd168xVariant,
};
pub use ssd2681::{Ssd2681Controller, Ssd2681RefreshMode};
pub use uc8151d::{Uc8151dController, Uc8151dRefreshMode};
pub use uc8175::Uc8175Controller;
pub use uc8253::{Uc8253Controller, Uc8253RefreshMode, Uc8253Variant};

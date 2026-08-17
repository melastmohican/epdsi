//! Controller IC module for various EPD display drivers.

pub mod ed2208;
pub mod jd79661;
pub mod pervasive_bw;
pub mod pervasive_bwry;
pub mod ssd1677;
pub mod ssd1680;
pub mod ssd1681;
pub mod uc8253;

pub use ed2208::Ed2208Controller;
pub use jd79661::Jd79661Controller;
pub use pervasive_bw::{PervasiveBwController, PervasiveDriverVariant, PervasiveRefreshMode};
pub use pervasive_bwry::{PervasiveBwryController, PervasiveBwryOtpError, PervasiveBwryVariant};
pub use ssd1677::{Ssd1677Controller, Ssd1677RefreshMode};
pub use ssd1680::{Ssd1680Controller, Ssd1680RefreshMode};
pub use ssd1681::Ssd1681Controller;
pub use uc8253::{Uc8253Controller, Uc8253RefreshMode};

//! Controller IC module for various EPD display drivers.

pub mod ed2208;
pub mod jd79661;
pub mod pervasive;
pub mod ssd1681;

pub use ed2208::Ed2208Controller;
pub use jd79661::Jd79661Controller;
pub use pervasive::{PervasiveDisplaysController, PervasiveDriverVariant, PervasiveRefreshMode};
pub use ssd1681::Ssd1681Controller;

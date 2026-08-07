//! Controller IC module for various EPD display drivers.

pub mod jd79661;
pub mod ssd1681;

pub use jd79661::Jd79661Controller;
pub use ssd1681::Ssd1681Controller;

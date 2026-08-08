//! Controller IC module for various EPD display drivers.

pub mod jd79661;
pub mod pervasive;
pub mod ssd1681;

pub use jd79661::Jd79661Controller;
pub use pervasive::PervasiveDisplaysController;
pub use ssd1681::Ssd1681Controller;

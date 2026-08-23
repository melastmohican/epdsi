//! # `epdsi` - E-Paper Display Serial Interface Framework
//!
//! `epdsi` is a no_std, `embedded-hal` 1.0 compatible Rust framework for Electronic Paper Displays (EPDs).
//! It decouples physical display panel specs (`EpdPanel`), driver IC register maps (`EpdController`),
//! and physical communication hardware (`SpiBusWrapper`).

#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![deny(missing_docs)]
#![deny(unsafe_code)]

pub mod bus;
pub mod bus3;
pub mod controllers;
pub mod driver;
pub mod graphics;
pub mod panels;
pub mod prelude;
pub mod traits;

pub use bus::SpiBusWrapper;
pub use bus3::Spi3Bus;
pub use driver::{EpdBuilder, EpdDriver};
pub use traits::{ColorChannel, ColorMode, EpdController, EpdPanel, SevenColor};

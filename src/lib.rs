//! # `epdsi` — E-Paper Display Serial Interface Framework
//!
//! A `no_std`, [`embedded-hal`] 1.0 compatible driver framework for Electronic Paper
//! Displays (EPDs), covering seven driver ICs and twelve panels behind one API.
//!
//! Most EPD crates bind one driver IC to one panel. `epdsi` separates the two, so adding
//! a panel to an existing controller is a single new file, and adding a controller does
//! not disturb the panels already supported.
//!
//! # Architecture
//!
//! Four pieces compose into a driver:
//!
//! - [`EpdPanel`] — a zero-sized type holding static physical facts about one panel:
//!   [`WIDTH`](EpdPanel::WIDTH), [`HEIGHT`](EpdPanel::HEIGHT),
//!   [`COLOR_MODE`](EpdPanel::COLOR_MODE), plus optional
//!   [`vcom`](EpdPanel::vcom), [`custom_lut`](EpdPanel::custom_lut) and
//!   [`gate_voltage`](EpdPanel::gate_voltage) overrides. See [`panels`].
//! - [`EpdController`] — the driver IC's command and register logic: init sequence,
//!   window and cursor addressing, frame writes, refresh, sleep. See [`controllers`].
//! - [`SpiBusWrapper`] — the physical transport, wrapping an `embedded-hal` `SpiDevice`
//!   plus DC, RST and BUSY pins. See [`bus`]. Panels needing OTP register reads over a
//!   bit-banged 3-wire link use [`Spi3Bus`] instead (see [`bus3`]).
//! - [`EpdDriver`] — the orchestrator, built with [`EpdBuilder`], exposing the public
//!   API: `init`, `set_window`, `write_frame`, `clear_frame`, `refresh`, `sleep`.
//!
//! Colour is unified across all panels by [`ColorMode`] and [`ColorChannel`], so
//! multi-buffer COGs (Pervasive's separate black/white and red RAM, for instance) are
//! always addressed explicitly rather than implicitly. Panels using the 4 bpp ACeP /
//! Spectra palette pack two pixels per byte through [`SevenColor::pack`].
//!
//! # Supported controllers and panels
//!
//! | Controller | Panels | Resolution | Colour mode |
//! | :--- | :--- | :--- | :--- |
//! | [`Ssd1681Controller`] | [`GDEM0154Z90`] | 200 × 200 | Tri-Color |
//! | [`Ssd1680Controller`] | [`GDEM0213B74`], [`GDEY0266Z90`] | 122 × 250, 152 × 296 | Monochrome, Tri-Color |
//! | [`Jd79661Controller`] | [`ZJY122250_0213AJH_E5`] / [`GDEY0213F51`] | 122 × 250 | Quad-Color |
//! | [`Uc8253Controller`] | [`GDEY037T03`], [`SE0352N14TNGA0`] | 240 × 416, 240 × 360 | Monochrome, Tri-Color |
//! | [`Ssd1677Controller`] | [`GDEQ0426T82`] | 800 × 480 | Monochrome |
//! | [`Ed2208Controller`] | [`GDEP073E01`] | 800 × 480 | Spectra 6 (4 bpp) |
//! | [`PervasiveBwController`] | [`E2266KS0C1`], [`E2290KS0F1`] | 152 × 296, 168 × 384 | Monochrome |
//! | [`PervasiveBwryController`] | [`E2154QS0F1`], [`E2417QS0A3`] | 152 × 152, 400 × 300 | Quad-Color (Spectra-4) |
//!
//! [`Ssd1680Controller`] and [`Ssd1681Controller`] are thin wrappers over the shared
//! [`Ssd168xController`]. [`Uc8253Controller`] carries two panel register profiles selected by
//! `Uc8253Variant`, since the two UC8253 panels disagree on init, RAM plane order and refresh —
//! [`SE0352N14TNGA0`] needs `Uc8253Variant::Se0352n14`. Many panels also carry
//! vendor-parity aliases, such as
//! `EPD_266_KS_0C` for [`E2266KS0C1`] or `GxEPD2_370_GDEY037T03` for [`GDEY037T03`].
//!
//! # Quick start
//!
//! ```rust,no_run
//! # use embedded_hal_mock::eh1::{
//! #     spi::Mock as SpiMock, digital::Mock as PinMock, delay::NoopDelay,
//! # };
//! # let spi_device = SpiMock::<u8>::new(&[]);
//! # let (dc_pin, rst_pin, busy_pin) =
//! #     (PinMock::new(&[]), PinMock::new(&[]), PinMock::new(&[]));
//! # let mut delay = NoopDelay;
//! # #[cfg(feature = "graphics")] {
//! use epdsi::prelude::*;
//! use embedded_graphics::{
//!     prelude::*, primitives::{Rectangle, PrimitiveStyle},
//!     pixelcolor::BinaryColor, geometry::{Point, Size},
//! };
//!
//! // Wrap the SPI device and its control pins.
//! let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
//! let controller = Ssd1681Controller::new(GDEM0154Z90::WIDTH, GDEM0154Z90::HEIGHT);
//!
//! // Bind controller and panel into a driver.
//! let mut epd = EpdBuilder::<_, GDEM0154Z90>::new(controller).build(epd_bus);
//! epd.init(&mut delay).unwrap();
//!
//! // Both RAM channels must be primed on a tri-colour panel.
//! epd.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
//! epd.clear_frame(ColorChannel::RedYellow, 0x00).unwrap();
//!
//! // Draw through embedded-graphics into a PageBuffer.
//! let mut bw_buf = [0xFFu8; 200 * 200 / 8];
//! let mut display = PageBuffer::new(&mut bw_buf, 200, 200, 0);
//! Rectangle::new(Point::new(10, 10), Size::new(50, 50))
//!     .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
//!     .draw(&mut display)
//!     .unwrap();
//!
//! epd.write_frame(ColorChannel::BlackWhite, display.as_slice()).unwrap();
//! epd.refresh(&mut delay).unwrap();
//! # }
//! ```
//!
//! [`prelude`] re-exports everything above and is the intended single import.
//!
//! # Low-RAM paged rendering
//!
//! A full 800 × 480 monochrome frame is 48 KB — more than many targets have. Rather than
//! buffering a whole frame, [`render_paged`](graphics::render_paged) sweeps the panel one
//! horizontal band at a time, handing a small stack-allocated [`PageBuffer`] to a closure
//! for each band, writing it, and refreshing once at the end. This is the GxEPD2 paged
//! pattern; RAM use is set by the page height you choose, not by panel size.
//!
//! # Colour panels refresh slowly, and that is physics
//!
//! Tri-Color and Quad-Color panels have no fast differential waveform. The coloured
//! pigment is a heavier particle needing the full OTP waveform to migrate, so *every*
//! update takes seconds — roughly 14 s on [`GDEM0154Z90`]. Partial refresh modes select a
//! controller LUT that only exists for monochrome panels; on a colour panel it produces
//! wrong output rather than a fast update. Partial *window* updates still work, at full
//! refresh speed.
//!
//! # Cargo features
//!
//! - `graphics` *(default)* — implements `embedded-graphics-core`'s `DrawTarget` and
//!   `Dimensions` for [`PageBuffer`]. Disable to drop the dependency; the buffer and
//!   paged rendering still work, you just fill pixels yourself.
//! - `defmt` — derives `defmt::Format` on the public error and mode enums for logging on
//!   embedded targets.
//!
//! # Hardware note
//!
//! On EXT3-1 extension boards, the **J3 jumper must be OPEN** (10 µH path) for panels
//! 3.7" and smaller. Closed (47 µH) the DC-DC booster sags during current bursts, which
//! shows up as BUSY-pin hangs that look like driver bugs but are not.
//!
//! # Complete examples
//!
//! Runnable, flashable programs for every supported controller live in
//! [`rust-rpico2-discovery`] (RP2350 Pico 2, `rp-hal`) and
//! [`rust-reterminal-e1002-examples`] (XIAO ESP32-S3, Embassy + `esp-hal`).
//!
//! The minimum supported Rust version is 1.75.
//!
//! [`Ssd168xController`]: controllers::Ssd168xController
//! [`Ssd1680Controller`]: controllers::Ssd1680Controller
//! [`Ssd1681Controller`]: controllers::Ssd1681Controller
//! [`Ssd1677Controller`]: controllers::Ssd1677Controller
//! [`Uc8253Controller`]: controllers::Uc8253Controller
//! [`Jd79661Controller`]: controllers::Jd79661Controller
//! [`Ed2208Controller`]: controllers::Ed2208Controller
//! [`PervasiveBwController`]: controllers::PervasiveBwController
//! [`PervasiveBwryController`]: controllers::PervasiveBwryController
//! [`GDEM0154Z90`]: panels::GDEM0154Z90
//! [`GDEM0213B74`]: panels::GDEM0213B74
//! [`GDEY0266Z90`]: panels::GDEY0266Z90
//! [`ZJY122250_0213AJH_E5`]: panels::ZJY122250_0213AJH_E5
//! [`GDEY0213F51`]: panels::GDEY0213F51
//! [`GDEY037T03`]: panels::GDEY037T03
//! [`SE0352N14TNGA0`]: panels::SE0352N14TNGA0
//! [`GDEQ0426T82`]: panels::GDEQ0426T82
//! [`GDEP073E01`]: panels::GDEP073E01
//! [`E2266KS0C1`]: panels::E2266KS0C1
//! [`E2290KS0F1`]: panels::E2290KS0F1
//! [`E2154QS0F1`]: panels::E2154QS0F1
//! [`E2417QS0A3`]: panels::E2417QS0A3
//! [`PageBuffer`]: graphics::PageBuffer
//! [`embedded-hal`]: https://docs.rs/embedded-hal/1.0.0/embedded_hal/
//! [`rust-rpico2-discovery`]: https://github.com/melastmohican/rust-rpico2-discovery
//! [`rust-reterminal-e1002-examples`]: https://github.com/melastmohican/rust-reterminal-e1002-examples

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

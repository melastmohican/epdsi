//! # `epdsi` — E-Paper Display Serial Interface Framework
//!
//! A `no_std`, [`embedded-hal`] 1.0 compatible driver framework for Electronic Paper
//! Displays (EPDs), covering fifteen controllers and twenty panels behind one API — blocking or
//! async, see the "Cargo features" section below.
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
//!   [`VCOM`](EpdPanel::VCOM), [`CUSTOM_LUT`](EpdPanel::CUSTOM_LUT) and
//!   [`GATE_VOLTAGE`](EpdPanel::GATE_VOLTAGE) overrides. See [`panels`].
//! - [`EpdController`] — the driver IC's command and register logic: init sequence,
//!   window and cursor addressing, frame writes, refresh, sleep. See [`controllers`].
//! - [`SpiBusWrapper`] — the physical transport, wrapping an `embedded-hal` `SpiDevice`
//!   plus DC, RST and BUSY pins. See [`bus`]. Panels needing OTP register reads over a
//!   bit-banged 3-wire link use [`Spi3Bus`] instead (see [`bus3`]).
//! - [`EpdDriver`] — the orchestrator, built with [`EpdBuilder`], exposing the public
//!   API: `init`, `set_window`, `write_frame`, `clear_frame`, `refresh`, `sleep`, plus
//!   [`display_frame`](EpdDriver::display_frame) bundling a single `write_frame` + `refresh`
//!   for the common single-channel case.
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
//! | [`Ssd1681Controller`] | [`GDEM0154Z90`] / [`GxEPD2_154_Z90c`] | 200 × 200 | Tri-Color |
//! | [`Ssd1680Controller`] | [`GDEM0213B74`], [`GDEY0266Z90`], [`GDEY0266T90`] | 122 × 250, 152 × 296, 152 × 296 | Monochrome, Tri-Color |
//! | [`Ssd168xController`] with [`Ssd168xVariant::Ssd1685`] | [`GDEY0266T90H`] | 184 × 360 | Monochrome |
//! | [`Jd79661Controller`] | [`ZJY122250_0213AJH_E5`] / [`GDEY0213F51`] / [`GxEPD2_213c_GDEY0213F51`] | 122 × 250 | Quad-Color |
//! | [`Jd79676Controller`] | [`GDEY0213F52`] | 122 × 250 | Quad-Color |
//! | [`Ist7163Controller`] | [`GDEM037F51`] | 240 × 416 | Quad-Color |
//! | [`Jd79660Controller`] | [`GDEM0154F51H`] / [`GxEPD2_154c_GDEM0154F51H`] | 200 × 200 | Quad-Color |
//! | [`Uc8253Controller`] | [`GDEY037T03`], [`SE0352N14TNGA0`] | 240 × 416, 240 × 360 | Monochrome, Tri-Color |
//! | [`Uc8151dController`] | [`GDEW0215T12`] | 112 × 208 | Monochrome |
//! | [`Uc8175Controller`] | [`GDEW0102I4FC`] | 80 × 128 | Monochrome |
//! | [`Ssd2681Controller`] | [`GDEM0154F61H`] | 200 × 200 | Quad-Color |
//! | [`Ssd1677Controller`] | [`GDEQ0426T82`] / [`GxEPD2_426_GDEQ0426T82`] | 800 × 480 | Monochrome |
//! | [`Ed2208Controller`] | [`GDEP073E01`] | 800 × 480 | Spectra 6 (4 bpp) |
//! | [`PervasiveBwController`] | [`E2266KS0C1`], [`E2290KS0F1`] | 152 × 296, 168 × 384 | Monochrome |
//! | [`PervasiveBwryController`] | [`E2154QS0F1`], [`E2417QS0A3`] | 152 × 152, 400 × 300 | Quad-Color (Spectra-4) |
//!
//! [`Ssd1680Controller`] and [`Ssd1681Controller`] are thin wrappers over the shared
//! [`Ssd168xController`], and [`Jd79660Controller`]/[`Jd79661Controller`]/[`Jd79676Controller`]
//! are the same relationship over [`Jd7966xController`] — the JD ICs share one SPI register table, differing
//! only in which registers `Jd7966xVariant` selects at init. [`Uc8253Controller`] carries two
//! panel register profiles selected by
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
//! # // `embedded_hal_mock`'s mocks are blocking-only, so this example only compiles in
//! # // blocking mode; see the crate's `blocking` feature doc for the async equivalent shape.
//! # #[cfg(all(feature = "graphics", feature = "blocking"))] {
//! use epdsi::prelude::*;
//! use embedded_graphics::{
//!     prelude::*, primitives::{Rectangle, PrimitiveStyle},
//!     geometry::{Point, Size},
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
//! // GDEM0154Z90 is a Tri-Color panel: draw both RAM planes together through one
//! // `PageBufferPair`, addressed by `TriColor` instead of two separate `PageBuffer`s and
//! // their disagreeing raw bit polarities. `PlanePolarity::SSD168X` is the constant this
//! // controller family needs — see its docs if porting to a different controller.
//! let mut bw_buf = [0xFFu8; 200 * 200 / 8];
//! let mut accent_buf = [0x00u8; 200 * 200 / 8];
//! let mut display = PageBufferPair::new(
//!     &mut bw_buf, &mut accent_buf, 200, 200, 0, PlanePolarity::SSD168X,
//! );
//! Rectangle::new(Point::new(10, 10), Size::new(50, 50))
//!     .into_styled(PrimitiveStyle::with_fill(TriColor::Accent))
//!     .draw(&mut display)
//!     .unwrap();
//!
//! epd.write_frame(ColorChannel::BlackWhite, display.bw().as_slice()).unwrap();
//! epd.write_frame(ColorChannel::RedYellow, display.accent().as_slice()).unwrap();
//! epd.refresh(&mut delay).unwrap();
//! # }
//! ```
//!
//! [`prelude`] re-exports everything above and is the intended single import. For a
//! monochrome-only panel, draw into a single [`PageBuffer`] with `embedded-graphics`'
//! `BinaryColor` instead — see the "Low-RAM paged rendering" section below.
//!
//! # Low-RAM paged rendering
//!
//! A full 800 × 480 monochrome frame is 48 KB — more than many targets have. Rather than
//! buffering a whole frame, [`render_paged`](graphics::render_paged) sweeps the panel one
//! horizontal band at a time, handing a small stack-allocated [`PageBuffer`] to a closure
//! for each band, writing it, and refreshing once at the end. This is the GxEPD2 paged
//! pattern; RAM use is set by the page height you choose, not by panel size.
//!
//! [`render_paged_tri_color`](graphics::render_paged_tri_color) is the same sweep for
//! Tri-Color panels, handing the closure a [`PageBufferPair`] per page instead of a single
//! [`PageBuffer`] — the paged counterpart to the Quick Start example above.
//!
//! [`render_paged_gray4`](graphics::render_paged_gray4) is the same pattern again, for SSD168x
//! panels configured with Adafruit_EPD-sourced 4-level grayscale mode (`GDEY0266T90` today —
//! see [`Gray4Registers`](traits::Gray4Registers)'s doc for the "not vendor material" caveat),
//! handing the closure a [`GrayBufferPair`] per page.
//!
//! [`render_paged_gray4_preclear`](graphics::render_paged_gray4_preclear) is the two-pass
//! counterpart for `GDEQ0426T82`'s Gray4 mode (`Ssd1677Controller`) — that chip's grayscale
//! refresh needs a baseline mono pass and an LUT reload between two refreshes, which the
//! single-pass `render_paged_gray4` cannot express, so this is a dedicated function rather than a
//! generic one.
//!
//! [`graphics::dither`] turns `Rgb888` pixels into the palette a panel can show with Bayer
//! ordered dithering. The functions are stateless per-pixel calls, so they need no extra buffer
//! inside a paged closure.
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
//! # Writing your own panel or controller
//!
//! If the combination you need is not in the table above, you do not have to wait for a
//! release. [`EpdPanel`] and [`EpdController`] are ordinary public traits, so a panel or a driver
//! IC can live in your own crate and plug into [`EpdBuilder`] exactly like the shipped ones.
//!
//! | You have | You write | You reuse |
//! | :--- | :--- | :--- |
//! | A new panel on a driver IC `epdsi` already supports | an [`EpdPanel`], three constants | the shipped controller |
//! | A driver IC `epdsi` does not support | an [`EpdController`] and an [`EpdPanel`] | [`SpiBusWrapper`], [`EpdBuilder`], paged rendering |
//! | A driver IC that is a near-sibling of a shipped one | in this crate, a variant on that controller, as [`Ssd168xVariant::Ssd1685`] is on [`Ssd168xController`] | everything else |
//! | The same, from your own crate | a wrapper that holds the shipped controller and overrides only the methods that differ | the rest of its methods |
//!
//! ## A new panel on a shipped controller
//!
//! A panel is a zero-sized type holding static facts. Nothing else is needed when the vendor's
//! init matches a controller `epdsi` already ships:
//!
//! ```rust
//! use epdsi::prelude::*;
//!
//! /// A hypothetical 2.13" tri-colour panel behind an SSD1680.
//! #[derive(Debug, Clone, Copy, Default)]
//! pub struct MyPanel;
//!
//! impl EpdPanel for MyPanel {
//!     const WIDTH: u32 = 122;
//!     const HEIGHT: u32 = 250;
//!     const COLOR_MODE: ColorMode = ColorMode::TriColor;
//!     // Declare these only if the vendor's reference driver writes them for this panel.
//!     // const VCOM: Option<u8> = Some(0x28);
//! }
//!
//! // `for_panel` reads WIDTH, HEIGHT and the optional overrides from the panel type.
//! let _controller = Ssd1680Controller::for_panel::<MyPanel>();
//! ```
//!
//! ## A shipped controller that differs by a few bytes
//!
//! Sometimes the new panel's IC is a sibling of one `epdsi` ships, with the same command set and
//! a handful of different bytes. The SSD1685 behind the [`GDEY0266T90H`] was one. Next to the
//! SSD1681 profile it differs only in the partial update byte (`0xDC` against `0xFC`) and the
//! temperature override that `FastFull` writes (`6E 00` against `5A 00`). It did not need a new
//! controller. It became [`Ssd168xVariant::Ssd1685`] and a panel file.
//!
//! Inside this crate that is the pattern: add a variant, branch on it where the bytes differ, and
//! pin each difference in a test. Two things to know:
//!
//! - The controller never sees the panel type, so pairing a panel with the wrong variant does
//!   not error. It sends the wrong bytes and renders badly. Name the pairing in the panel's
//!   module doc and in the variant's doc.
//! - Write the differences down as a table with the source of each byte: vendor demo,
//!   datasheet, or another driver. The doc on [`Ssd168xVariant::Ssd1685`] is an example.
//!
//! From your own crate you cannot add a variant to someone else's enum. Hold the shipped
//! controller instead, override the methods whose bytes differ, and delegate the rest. This one
//! reuses the SSD1681 profile and changes only the partial update trigger:
//!
//! ```rust
//! # #[cfg(feature = "blocking")]
//! # use embedded_hal::delay::DelayNs;
//! # #[cfg(not(feature = "blocking"))]
//! # use embedded_hal_async::delay::DelayNs;
//! # #[cfg(feature = "blocking")]
//! # use embedded_hal::spi::SpiDevice;
//! # #[cfg(not(feature = "blocking"))]
//! # use embedded_hal_async::spi::SpiDevice;
//! # use embedded_hal::digital::{InputPin, OutputPin};
//! # #[cfg(feature = "blocking")]
//! # use embedded_hal::digital::InputPin as Wait;
//! # #[cfg(not(feature = "blocking"))]
//! # use embedded_hal_async::digital::Wait;
//! use epdsi::bus::{EpdBusError, SpiBusWrapper};
//! use epdsi::controllers::{Ssd168xController, Ssd168xRefreshMode};
//! use epdsi::traits::{ColorChannel, EpdController};
//!
//! /// An SSD1681 sibling whose partial update byte is `0xDC`.
//! pub struct MySibling {
//!     inner: Ssd168xController,
//! }
//!
//! #[maybe_async_cfg::maybe(
//!     sync(feature = "blocking", keep_self),
//!     async(not(feature = "blocking"), keep_self)
//! )]
//! impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for MySibling
//! where
//!     SPI: SpiDevice,
//!     DC: OutputPin,
//!     RST: OutputPin,
//!     BUSY: InputPin + Wait,
//! {
//!     type Error = EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;
//!
//!     async fn init_sequence<DELAY: DelayNs>(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         delay: &mut DELAY,
//!     ) -> Result<(), Self::Error> {
//!         self.inner.init_sequence(bus, delay).await
//!     }
//!
//!     async fn set_window(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         x_start: u32,
//!         y_start: u32,
//!         x_end: u32,
//!         y_end: u32,
//!     ) -> Result<(), Self::Error> {
//!         self.inner.set_window(bus, x_start, y_start, x_end, y_end).await
//!     }
//!
//!     async fn set_cursor(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         x: u32,
//!         y: u32,
//!     ) -> Result<(), Self::Error> {
//!         self.inner.set_cursor(bus, x, y).await
//!     }
//!
//!     async fn write_frame(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         channel: ColorChannel,
//!         data: &[u8],
//!     ) -> Result<(), Self::Error> {
//!         self.inner.write_frame(bus, channel, data).await
//!     }
//!
//!     async fn write_frame_pattern(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         channel: ColorChannel,
//!         byte: u8,
//!         count: usize,
//!     ) -> Result<(), Self::Error> {
//!         self.inner.write_frame_pattern(bus, channel, byte, count).await
//!     }
//!
//!     async fn trigger_refresh<DELAY: DelayNs>(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         delay: &mut DELAY,
//!     ) -> Result<(), Self::Error> {
//!         if self.inner.refresh_mode() == Ssd168xRefreshMode::Partial {
//!             // The only bytes this IC changes: the shipped SSD1681 profile sends `0xFC` here.
//!             bus.send_command_with_data(0x22, &[0xDC]).await?;
//!             bus.send_command(0x20).await?;
//!             bus.wait_busy(true).await
//!         } else {
//!             self.inner.trigger_refresh(bus, delay).await
//!         }
//!     }
//!
//!     async fn sleep<DELAY: DelayNs>(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         delay: &mut DELAY,
//!     ) -> Result<(), Self::Error> {
//!         self.inner.sleep(bus, delay).await
//!     }
//! }
//!
//! // Build the inner controller as you would for the shipped IC, then wrap it:
//! // `MySibling { inner: Ssd168xController::new_ssd1681(184, 360)
//! //     .with_refresh_mode(Ssd168xRefreshMode::Partial) }`
//! ```
//!
//! The same `blocking` feature rule applies as for a full controller below.
//!
//! ## Your own controller
//!
//! [`EpdController`] has seven methods: `init_sequence`, `set_window`, `set_cursor`,
//! `write_frame`, `write_frame_pattern`, `trigger_refresh` and `sleep`. The bus helpers on
//! [`SpiBusWrapper`] (`hard_reset`, `send_command`, `send_command_with_data`,
//! `send_data_repeated`, `wait_busy`) cover everything a register-level driver needs.
//!
//! You do not need variants. Variants such as [`Ssd168xVariant`] are how `epdsi`'s own controllers
//! share code between related ICs, and nothing in [`EpdController`], [`EpdPanel`] or
//! [`EpdBuilder`] refers to them. A driver for one IC is a single struct with its init bytes
//! inline, as in the example below.
//!
//! The trait is declared through `maybe_async_cfg`, so its methods are plain functions in
//! blocking mode and `async fn` in async mode. A crate outside `epdsi` has to follow the same
//! switch. Give your crate a feature named exactly `blocking` that forwards to `epdsi`, and take
//! `epdsi` with its default features off:
//!
//! ```toml
//! [dependencies]
//! epdsi = { version = "0.9", default-features = false }
//! embedded-hal = "1.0"
//! embedded-hal-async = "1.0"
//! maybe-async-cfg = "0.2"
//!
//! [features]
//! default = ["blocking"]
//! blocking = ["epdsi/blocking"]   # the name must be `blocking`
//! ```
//!
//! With any other feature name, `maybe_async_cfg` picks the async shape while `epdsi` is
//! blocking. The compiler then reports `E0053`, "method `init_sequence` has an incompatible
//! type for trait ... expected `Result<..>`, found future", which does not mention the feature.
//!
//! The example below is a complete controller for an imaginary IC. It compiles in both modes,
//! and `cargo test --doc` runs it in CI under each:
//!
//! ```rust
//! # #[cfg(feature = "blocking")]
//! # use embedded_hal::delay::DelayNs;
//! # #[cfg(not(feature = "blocking"))]
//! # use embedded_hal_async::delay::DelayNs;
//! # #[cfg(feature = "blocking")]
//! # use embedded_hal::spi::SpiDevice;
//! # #[cfg(not(feature = "blocking"))]
//! # use embedded_hal_async::spi::SpiDevice;
//! # use embedded_hal::digital::{InputPin, OutputPin};
//! # #[cfg(feature = "blocking")]
//! # use embedded_hal::digital::InputPin as Wait;
//! # #[cfg(not(feature = "blocking"))]
//! # use embedded_hal_async::digital::Wait;
//! use epdsi::bus::{EpdBusError, SpiBusWrapper};
//! use epdsi::traits::{ColorChannel, ColorMode, EpdController, EpdPanel};
//!
//! /// Static facts about the glass.
//! #[derive(Debug, Clone, Copy, Default)]
//! pub struct MyPanel;
//!
//! impl EpdPanel for MyPanel {
//!     const WIDTH: u32 = 128;
//!     const HEIGHT: u32 = 296;
//!     const COLOR_MODE: ColorMode = ColorMode::BlackWhite;
//! }
//!
//! /// Command and register logic for the driver IC.
//! pub struct MyController {
//!     height: u32,
//! }
//!
//! #[maybe_async_cfg::maybe(
//!     sync(feature = "blocking", keep_self),
//!     async(not(feature = "blocking"), keep_self)
//! )]
//! impl<SPI, DC, RST, BUSY> EpdController<SpiBusWrapper<SPI, DC, RST, BUSY>> for MyController
//! where
//!     SPI: SpiDevice,
//!     DC: OutputPin,
//!     RST: OutputPin,
//!     BUSY: InputPin + Wait,
//! {
//!     type Error = EpdBusError<SPI::Error, DC::Error, RST::Error, BUSY::Error>;
//!
//!     async fn init_sequence<DELAY: DelayNs>(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         delay: &mut DELAY,
//!     ) -> Result<(), Self::Error> {
//!         bus.hard_reset(delay, 10).await?;
//!         // `true` means BUSY is high while the panel works. Check your panel's polarity.
//!         bus.wait_busy(true).await?;
//!         bus.send_command_with_data(0x01, &[(self.height - 1) as u8, 0x00]).await
//!     }
//!
//!     async fn set_window(
//!         &mut self,
//!         _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         _x_start: u32,
//!         _y_start: u32,
//!         _x_end: u32,
//!         _y_end: u32,
//!     ) -> Result<(), Self::Error> {
//!         Ok(()) // full-frame streaming only
//!     }
//!
//!     async fn set_cursor(
//!         &mut self,
//!         _bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         _x: u32,
//!         _y: u32,
//!     ) -> Result<(), Self::Error> {
//!         Ok(())
//!     }
//!
//!     async fn write_frame(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         _channel: ColorChannel,
//!         data: &[u8],
//!     ) -> Result<(), Self::Error> {
//!         bus.send_command_with_data(0x24, data).await
//!     }
//!
//!     async fn write_frame_pattern(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         _channel: ColorChannel,
//!         byte: u8,
//!         count: usize,
//!     ) -> Result<(), Self::Error> {
//!         bus.send_command(0x24).await?;
//!         bus.send_data_repeated(byte, count).await
//!     }
//!
//!     async fn trigger_refresh<DELAY: DelayNs>(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         _delay: &mut DELAY,
//!     ) -> Result<(), Self::Error> {
//!         bus.send_command(0x20).await?;
//!         bus.wait_busy(true).await
//!     }
//!
//!     async fn sleep<DELAY: DelayNs>(
//!         &mut self,
//!         bus: &mut SpiBusWrapper<SPI, DC, RST, BUSY>,
//!         _delay: &mut DELAY,
//!     ) -> Result<(), Self::Error> {
//!         bus.send_command_with_data(0x10, &[0x01]).await
//!     }
//! }
//!
//! // Bind it exactly like a shipped controller:
//! // `EpdBuilder::<MyController, MyPanel>::new(MyController { height: 296 }).build(bus)`
//! ```
//!
//! ## Before you power the panel
//!
//! Every hardware problem this project has hit sat outside the register values themselves, so
//! check these first:
//!
//! - **Reset and BUSY polarity.** Panels differ, and a wrong `wait_busy` argument either hangs
//!   or returns at once with nothing drawn.
//! - **Reset pulse width.** Take the IC datasheet's minimum, then the vendor demo's value. A
//!   pulse at the minimum has latched intermittently on some hosts.
//! - **Pixel packing and RAM stride.** Know bits per pixel, which bit is the first pixel, and
//!   whether the row is padded to a byte or to four pixels.
//! - **Power sequence.** Some controllers drop their charge pump after a refresh, so a bare
//!   refresh command is silently ignored until `PON` is sent again. The reverse also happens:
//!   a vendor demo's bare `0x1C` partial trigger has no power-on bits and only works because the
//!   demo always runs a full update first. As the first update after init it would do nothing.
//! - **Registers the demo leaves alone.** If the vendor demo never writes a register, do not
//!   write it either. The panel's OTP may already have set it, and your write replaces that
//!   value. Writing the SSD1685's source-count register with its power-on default selected 200
//!   sources on a 184-source panel and drew a noise strip down one edge.
//! - **The previous-image plane.** Many monochrome controllers keep a second RAM plane as the
//!   "previous frame" for differential updates. Check what the demo writes there before a full
//!   update (the SSD1685 demo writes `0x00`) and what it copies in afterwards. The order
//!   decides whether the next partial update diffs against the right base.
//! - **Sleep.** Deep sleep usually wakes only through a hardware reset, so `init_sequence`
//!   must start with one.
//!
//! Write the vendor demo's bytes down in order, compare them with the datasheet, and mark any
//! byte the datasheet does not document as vendor-only in a comment. Then pin the whole stream
//! in a test that records each SPI write together with the DC pin level, as the tests under
//! `tests/` in this repository do, so a later edit cannot change it unnoticed.
//!
//! ## Sending it upstream
//!
//! Before you open an issue asking for a new panel, read the
//! [panel support policy](https://github.com/melastmohican/epdsi#panel-support-policy). In
//! short, `epdsi` only adds panels the maintainer has in hand, never from a datasheet alone, so
//! that every parity claim is backed by a bench run. The maintainer buys new panels
//! occasionally, if they are available to buy, and accepts donations only on exception, for
//! panels from known sources that are well documented. Adding support takes as long as it
//! takes.
//!
//! If your panel fits that, open an issue with the part number, the vendor demo and the
//! datasheet. Adding it means a panel file under `src/panels/`, plus either a new file under
//! `src/controllers/` or a variant on the nearest shipped controller, then byte-exact tests and
//! a hardware example for blocking and async. Your own crate keeps working in the
//! meantime.
//!
//! # Cargo features
//!
//! - `blocking` *(default)* — the plain `embedded-hal` 1.0 API used throughout this doc.
//!   Disabling it (`default-features = false`) switches every controller/bus/driver method
//!   to its `embedded-hal-async` counterpart instead — the two are mutually exclusive, not
//!   additive, since a bus can only be one or the other:
//!
//!   ```toml
//!   epdsi = { version = "0.9", default-features = false, features = ["graphics"] }
//!   ```
//!
//!   The async shape mirrors the blocking one exactly (same types, same method names, every
//!   fallible method returns the same `Result` — just `.await`ed), generated from one source
//!   per controller via native async-fn-in-trait (stable since this crate's MSRV, 1.75), so
//!   there is no separate API to learn. Two differences worth knowing before switching:
//!
//!   - [`SpiBusWrapper`]'s busy-wait methods poll with a bounded retry/timeout in blocking
//!     mode and return [`bus::EpdBusError::Timeout`] when it runs out, since a blocking spin has no
//!     other way to bound its own cost. In async mode they
//!     wait on the BUSY pin's edge via `embedded-hal-async`'s `Wait` trait instead, with **no
//!     timeout** — that trait has no timeout primitive to race against without an executor.
//!     Bound a call yourself if you need one, e.g. `embassy_time::with_timeout(..., epd.refresh(&mut delay))`.
//!   - [`render_paged`](graphics::render_paged)'s pixel-drawing closure stays synchronous in
//!     both modes — `embedded-graphics-core`'s `DrawTarget` has no async counterpart — only
//!     the I/O between pages is `.await`ed.
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
//! Runnable, flashable programs for every supported controller are verified across three
//! blocking hosts — [`rust-rpico2-discovery`] (RP2350 Pico 2, `rp-hal`),
//! `adafruit-feather-thinkink-discovery` (Feather RP2040 ThinkInk, `rp-hal`), and
//! `xiao-esp32c3-blinky` (XIAO ESP32-C3, `esp-hal`) — and two async hosts —
//! `rust-rpico2-embassy-examples` (RP2350 Pico 2, Embassy + `embassy-rp`) and
//! [`rust-reterminal-e1002-examples`] (XIAO ESP32-S3, Embassy + `esp-hal`). See the README's
//! "Examples on real hardware" section for the full per-panel verification table and links.
//!
//! The minimum supported Rust version is 1.75.
//!
//! [`Ssd168xController`]: controllers::Ssd168xController
//! [`Ssd168xVariant::Ssd1685`]: controllers::Ssd168xVariant::Ssd1685
//! [`Ssd168xVariant`]: controllers::Ssd168xVariant
//! [`Ssd1680Controller`]: controllers::Ssd1680Controller
//! [`Ssd1681Controller`]: controllers::Ssd1681Controller
//! [`Ssd1677Controller`]: controllers::Ssd1677Controller
//! [`Uc8253Controller`]: controllers::Uc8253Controller
//! [`Uc8151dController`]: controllers::Uc8151dController
//! [`Uc8175Controller`]: controllers::Uc8175Controller
//! [`Ssd2681Controller`]: controllers::Ssd2681Controller
//! [`Jd7966xController`]: controllers::Jd7966xController
//! [`Jd79660Controller`]: controllers::Jd79660Controller
//! [`Jd79661Controller`]: controllers::Jd79661Controller
//! [`Jd79676Controller`]: controllers::Jd79676Controller
//! [`Ist7163Controller`]: controllers::Ist7163Controller
//! [`Ed2208Controller`]: controllers::Ed2208Controller
//! [`PervasiveBwController`]: controllers::PervasiveBwController
//! [`PervasiveBwryController`]: controllers::PervasiveBwryController
//! [`GDEM0154Z90`]: panels::GDEM0154Z90
//! [`GxEPD2_154_Z90c`]: panels::GxEPD2_154_Z90c
//! [`GDEM0213B74`]: panels::GDEM0213B74
//! [`GDEY0266Z90`]: panels::GDEY0266Z90
//! [`GDEY0266T90`]: panels::GDEY0266T90
//! [`GDEY0266T90H`]: panels::GDEY0266T90H
//! [`ZJY122250_0213AJH_E5`]: panels::ZJY122250_0213AJH_E5
//! [`GDEY0213F51`]: panels::GDEY0213F51
//! [`GxEPD2_213c_GDEY0213F51`]: panels::GxEPD2_213c_GDEY0213F51
//! [`GDEY0213F52`]: panels::GDEY0213F52
//! [`GDEM037F51`]: panels::GDEM037F51
//! [`GDEM0154F51H`]: panels::GDEM0154F51H
//! [`GxEPD2_154c_GDEM0154F51H`]: panels::GxEPD2_154c_GDEM0154F51H
//! [`GDEY037T03`]: panels::GDEY037T03
//! [`GDEW0215T12`]: panels::GDEW0215T12
//! [`GDEW0102I4FC`]: panels::GDEW0102I4FC
//! [`GDEM0154F61H`]: panels::GDEM0154F61H
//! [`SE0352N14TNGA0`]: panels::SE0352N14TNGA0
//! [`GDEQ0426T82`]: panels::GDEQ0426T82
//! [`GxEPD2_426_GDEQ0426T82`]: panels::GxEPD2_426_GDEQ0426T82
//! [`GDEP073E01`]: panels::GDEP073E01
//! [`E2266KS0C1`]: panels::E2266KS0C1
//! [`E2290KS0F1`]: panels::E2290KS0F1
//! [`E2154QS0F1`]: panels::E2154QS0F1
//! [`E2417QS0A3`]: panels::E2417QS0A3
//! [`PageBuffer`]: graphics::PageBuffer
//! [`PageBufferPair`]: graphics::PageBufferPair
//! [`GrayBufferPair`]: graphics::GrayBufferPair
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
pub mod policy;
pub mod prelude;
pub mod traits;

pub use bus::SpiBusWrapper;
pub use bus3::Spi3Bus;
pub use driver::{EpdBuilder, EpdDriver};
pub use traits::{ColorChannel, ColorMode, EpdController, EpdPanel, SevenColor};

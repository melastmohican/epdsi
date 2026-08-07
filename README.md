# epdsi: E-Paper Display Serial Interface Framework

[![Crates.io](https://img.shields.io/crates/v/epdsi.svg)](https://crates.io/crates/epdsi)
[![Documentation](https://docs.rs/epdsi/badge.svg)](https://docs.rs/epdsi)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

A `no_std`, [`embedded-hal`](https://github.com/rust-embedded/embedded-hal) 1.0 compatible Rust driver framework for Electronic Paper Displays (EPD).

## Features

- **Modular Architecture**: Decouples driver IC logic (`EpdController`) from physical panel specifications (`EpdPanel`).
- **`embedded-hal` 1.0 Compatible**: Built around standard `SpiDevice`, `OutputPin`, `InputPin`, and `DelayNs` traits.
- **Low-RAM Paged Rendering**: Built-in support for GxEPD2-style closure-based paged graphics rendering using tiny stack buffers.
- **`embedded-graphics` Integration**: Implements `DrawTarget` and `Dimensions` via optional `graphics` feature (enabled by default).
- **Multi-Color Support**: Unified handling for Monochrome, Tri-Color (Black/White/Red), and 7-Color ACeP displays.

## Quick Start

Add `epdsi` to your `Cargo.toml`:

```toml
[dependencies]
epdsi = "0.1.0"
embedded-graphics = "0.8"
```

### Usage Example (SSD1681 Controller + GDEM0154Z90 Panel)

```rust,ignore
use epdsi::prelude::*;
use embedded_graphics::{prelude::*, primitives::Rectangle, pixelcolor::BinaryColor, geometry::Point, geometry::Size};

// Initialize SPI wrapper and controller
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Ssd1681Controller::new(GDEM0154Z90::WIDTH, GDEM0154Z90::HEIGHT);

// Build driver orchestrator
let mut epd = EpdBuilder::<_, GDEM0154Z90>::new(controller).build(epd_bus);

// Initialize display
epd.init(&mut delay).unwrap();

// Clear RAM channels
epd.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
epd.clear_frame(ColorChannel::RedYellow, 0x00).unwrap();

// Render graphics using PageBuffer
let mut bw_buf = [0xFFu8; (200 * 200 / 8) as usize];
let mut display = PageBuffer::new(&mut bw_buf, 200, 200, 0);

Rectangle::new(Point::new(10, 10), Size::new(50, 50))
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::On))
    .draw(&mut display)
    .unwrap();

// Send frame and refresh display
epd.write_frame(ColorChannel::BlackWhite, display.as_slice()).unwrap();
epd.refresh(&mut delay).unwrap();
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

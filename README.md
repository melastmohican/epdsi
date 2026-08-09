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
- **Multi-Color Support**: Unified handling for Monochrome, Tri-Color (Black/White/Red), Quad-Color (Black/White/Yellow/Red 2bpp), and 7-Color ACeP displays.
- **Automatic RAM Alignment**: Driver IC controllers automatically align panel widths to hardware byte boundaries (`div_ceil(8) * 8`).

## Supported Controllers & Panels

| Controller IC | Supported Panels | Resolution | Color Mode | Notes |
| :--- | :--- | :--- | :--- | :--- |
| **SSD1681** (`Ssd1681Controller`) | `GDEM0154Z90` | 200 × 200 | Tri-Color | 1.54" Tri-Color SPI panel |
| **JD79661** (`Jd79661Controller`) | `ZJY122250_0213AJH_E5` / `GDEY0213F51` | 122 × 250 | Quad-Color | 2.13" Quad-Color (Adafruit 6373/6366, Active-Low BUSY) |
| **ED2208** (`Ed2208Controller`) | `GDEP073E01` (`GxEPD2_730c_GDEP073E01`) | 800 × 480 | 7-Color ACeP | 7.3" 7-Color (Seeed reTerminal E1002, Waveshare PhotoPainter) |
| **Pervasive Displays** (`PervasiveDisplaysController`) | `E2266KS0C1` (`EPD_266_KS_0C`), `E2290KS0F1` (`EPD_290_KS_0F`) | 152 × 296, 168 × 384 | Monochrome | Pervasive Displays 2.66" (Driver C) & 2.90" (Driver F) Panels |

> **Hardware Note for EXT3-1 Extension Boards:** Ensure the **J3 jumper** is **OPEN** ($10\,\mu\text{H}$ inductor path) for panels $\le 3.7"$ (e.g. 2.66" and 2.9" panels). If J3 is closed ($47\,\mu\text{H}$ path), the DC-DC booster chokes during current bursts, causing voltage sags and BUSY pin hangs.

## Quick Start

Add `epdsi` to your `Cargo.toml`:

```toml
[dependencies]
epdsi = "0.1.0"
embedded-graphics = "0.8"
```

### 1. Usage Example (SSD1681 Controller + GDEM0154Z90 Panel)

```rust,ignore
use epdsi::prelude::*;
use embedded_graphics::{prelude::*, primitives::Rectangle, pixelcolor::BinaryColor, geometry::Point, geometry::Size};

// Initialize SPI bus wrapper and controller
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

### 2. Usage Example (JD79661 Controller + 2.13" Quad-Color Panel)

```rust,ignore
use epdsi::prelude::*;

// Initialize SPI bus wrapper and JD79661 controller
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);

// Build driver for Adafruit 6373 Quad-Color 122x250 display
let mut epd = EpdBuilder::<_, ZJY122250_0213AJH_E5>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();

// Send 2bpp packed QuadColor frame buffer (8,000 bytes for 128x250 hardware RAM)
epd.write_frame(ColorChannel::BlackWhite, &quad_color_frame_buf).unwrap();
epd.refresh(&mut delay).unwrap();
```

### 3. Usage Example (PervasiveDisplaysController + E2266KS0C1 Panel)

```rust,ignore
use epdsi::prelude::*;

// Initialize SPI bus wrapper and Pervasive Displays controller
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = PervasiveDisplaysController::new(E2266KS0C1::WIDTH, E2266KS0C1::HEIGHT);

// Build driver for Pervasive Displays 2.66" 152x296 Monochrome panel
let mut epd = EpdBuilder::<_, E2266KS0C1>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();

// Clear RAM channel and refresh
epd.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
epd.refresh(&mut delay).unwrap();
epd.sleep(&mut delay).unwrap();
```

### 4. Usage Example (ED2208 Controller + GDEP073E01 7-Color Panel)

```rust,ignore
use epdsi::prelude::*;

// Initialize SPI bus wrapper and ED2208 controller
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);

// Build driver for 7.3" 800x480 7-Color EPD display (e.g. Seeed reTerminal E1002)
let mut epd = EpdBuilder::<_, GDEP073E01>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();

// Clear display frame buffer (fill with White, 0x11)
epd.clear_frame(ColorChannel::SevenColor, SevenColor::pack(SevenColor::White, SevenColor::White)).unwrap();

// Send 4bpp packed 7-color frame buffer (192,000 bytes for 800x480)
epd.write_frame(ColorChannel::SevenColor, &seven_color_frame_buf).unwrap();
epd.refresh(&mut delay).unwrap();
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

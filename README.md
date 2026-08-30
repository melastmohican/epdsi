# epdsi: E-Paper Display Serial Interface Framework

[![Crates.io](https://img.shields.io/crates/v/epdsi.svg)](https://crates.io/crates/epdsi)
[![Documentation](https://img.shields.io/docsrs/epdsi)](https://docs.rs/epdsi)
[![CI](https://github.com/melastmohican/epdsi/actions/workflows/ci.yml/badge.svg)](https://github.com/melastmohican/epdsi/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE-MIT)

A `no_std`, [`embedded-hal`](https://github.com/rust-embedded/embedded-hal) 1.0 compatible Rust driver framework for Electronic Paper Displays (EPD).

![Panels driven by epdsi: GDEQ0426T82, GDEY037T03, ZJY122250, GDEM0213B74, GDEM0154Z90, E2417QS0A3, E2154QS0F1](epdsi.jpg)

*Every panel above driven by `epdsi` on an RP2350 Pico 2 — left to right: `GDEQ0426T82` (SSD1677),
`GDEY037T03` (UC8253), `ZJY122250` (JD79661), `GDEM0213B74` (SSD1680), `GDEM0154Z90` (SSD1681),
`E2417QS0A3` and `E2154QS0F1` (Pervasive Spectra-4 BWRY, Drivers A and F).*

## Features

- **Modular Architecture**: Decouples driver IC logic (`EpdController`) from physical panel specifications (`EpdPanel`).
- **`embedded-hal` 1.0 Compatible**: Built around standard `SpiDevice`, `OutputPin`, `InputPin`, and `DelayNs` traits.
- **Low-RAM Paged Rendering**: Built-in support for GxEPD2-style closure-based paged graphics rendering using tiny stack buffers.
- **`embedded-graphics` Integration**: Implements `DrawTarget` and `Dimensions` via optional `graphics` feature (enabled by default).
- **Multi-Color Support**: Unified handling for Monochrome, Tri-Color (Black/White/Red), Quad-Color (Black/White/Yellow/Red 2bpp), and 4bpp ACeP / E Ink Spectra palette displays.
- **Automatic RAM Alignment**: Driver IC controllers automatically align panel widths to hardware byte boundaries (`div_ceil(8) * 8`).

## Supported Controllers & Panels

| Controller IC | Supported Panels | Resolution | Color Mode | Notes |
| :--- | :--- | :--- | :--- | :--- |
| **SSD1681** (`Ssd1681Controller` / `Ssd168xController`) | `GDEM0154Z90` | 200 × 200 | Tri-Color | 1.54" Tri-Color SPI panel, Full refresh only (~14 s). `Ssd168xRefreshMode::Partial` is **not** usable — see [note below](#tri-color-panels-and-partial-refresh). Partial *window* updates work via `set_window` at full-refresh speed |
| **SSD1680(Z)** (`Ssd1680Controller` / `Ssd168xController`) | `GDEM0213B74`, `GDEY0266Z90` (`GxEPD2_266c`) | 122 × 250, 152 × 296 | Monochrome, Tri-Color | `GDEM0213B74`: 2.13" Monochrome (Adafruit 6383), Full/FastFull/Partial refresh. `GDEY0266Z90`: [Good Display GDEY0266Z90](https://www.good-display.com/product/430.html) / [Waveshare 2.66" e-Paper Module (B)](https://www.waveshare.com/2.66inch-e-Paper-B.htm), full refresh only (~18–20 s) — see [note below](#tri-color-panels-and-partial-refresh). Its Red RAM plane is **inverted** relative to the Black/White plane |
| **JD79661** (`Jd79661Controller`) | `ZJY122250_0213AJH_E5` / `GDEY0213F51` | 122 × 250 | Quad-Color | 2.13" Quad-Color ([Good Display GDEY0213F51](https://www.good-display.com/product/463.html), [Seeed Studio 5779](https://www.seeedstudio.com/2-13-Quadruple-Color-ePaper-Display-with-122x250-Pixels-p-5779.html), [Adafruit 6373](https://www.adafruit.com/product/6373), Active-Low BUSY) |
| **UC8253** (`Uc8253Controller`) | `GDEY037T03` (`GxEPD2_370_GDEY037T03`), `SE0352N14TNGA0` | 240 × 416, 240 × 360 | Monochrome, Tri-Color | Both Active-Low BUSY. `GDEY037T03`: 3.7" Monochrome (Adafruit 6395), Full/FastFull/Partial/FastPartial refresh. `SE0352N14TNGA0`: [Waveshare 3.52" e-Paper HAT (B)](https://www.waveshare.com/3.52inch-e-paper-hat-b.htm), full refresh only (~16–20 s), needs `Uc8253Variant::Se0352n14` — the two panels disagree on init, RAM plane order and ink polarity |
| **SSD1677** (`Ssd1677Controller`) | `GDEQ0426T82` | 800 × 480 | Monochrome | 4.26" Monochrome (Seeed Studio 6398, SE8350/SSD1677), Full/FastFull/Partial refresh |
| **ED2208** (`Ed2208Controller`) | `GDEP073E01` (`GxEPD2_730c_GDEP073E01`) | 800 × 480 | Spectra 6 (4bpp) | 7.3" six-colour E Ink Spectra 6 / `GDEP073E01(E6)` — black, white, red, yellow, blue, green. `SevenColor::Orange` is ACeP-7 only and **not** renderable here (Seeed reTerminal E1002) |
| **Pervasive Displays** (`PervasiveBwController`) | `E2266KS0C1` (`EPD_266_KS_0C`), `E2290KS0F1` (`EPD_290_KS_0F`) | 152 × 296, 168 × 384 | Monochrome | Pervasive Displays 2.66" (Driver C) & 2.90" (Driver F) Panels |
| **Pervasive Displays BWRY** (`PervasiveBwryController`) | `E2154QS0F1` (`EPD_154_QS_0F`), `E2417QS0A3` (`EPD_417_QS_0A`) | 152 × 152, 400 × 300 | Quad-Color (Spectra-4) | Pervasive Displays 1.54" (Driver F) & 4.2" (Driver A), OTP-sourced registers read via a bit-banged 3-wire handshake (`epdsi::bus3::Spi3Bus`), Active-Low BUSY |

> **Hardware Note for EXT3-1 Extension Boards:** Ensure the **J3 jumper** is **OPEN** ($10\,\mu\text{H}$ inductor path) for panels $\le 3.7"$ (e.g. 2.66" and 2.9" panels). If J3 is closed ($47\,\mu\text{H}$ path), the DC-DC booster chokes during current bursts, causing voltage sags and BUSY pin hangs.

### Tri-Color panels and partial refresh

Colour panels have **no fast/differential waveform**. The red (or yellow) pigment is a
heavier particle that needs the full OTP waveform to migrate, so *every* update on a
Tri-Color or Quad-Color panel takes seconds — roughly 14 s on the `GDEM0154Z90`, 18–20 s on the
`GDEY0266Z90`, and 16–20 s on the `SE0352N14TNGA0`, which for that reason exposes no partial mode
at all (`Uc8253RefreshMode` is ignored under `Uc8253Variant::Se0352n14`).

`Ssd168xRefreshMode::Partial` drives `UPDATE_DISPLAY_CTRL2 = 0xFC`, selecting the
controller's built-in fast LUT. That LUT only exists for monochrome panels. On a colour
panel it is **not** a speed-up and actively breaks the image: the update runs at full-refresh
speed anyway, and because the fast path only rewrites the Black/White RAM, all red content
is dropped. Keep colour panels on `Ssd168xRefreshMode::Full`.

`Ssd168xRefreshMode::FastFull` (`0xC7`, preceded by a `0x5A` temperature-register override that
reloads the OTP LUT) does help, but how much depends on the glass rather than the controller —
**measure it**. On a `GDEY0266Z90` it came out at 16.2 s against 20.0 s for `Full`, a 19 % saving,
on DKE glass; Good Display quote only ~19 s against ~20 s for their own. Same IC, same resolution,
different OTP waveform. No colour panel approaches the sub-second figures a monochrome SSD168x
panel reaches, because the red pigment has no differential waveform to skip.

`Ssd168xRefreshMode::BaseMap` (`0xF4`) primes the controller's previous-frame buffer before a run
of `Partial` updates. That is a **monochrome-only** workflow: on a colour panel `0x26` is always
the colour plane, never a previous-frame buffer, so seeding it with a Black/White image — correct
on the `GDEM0213B74` — sets nearly every bit and renders the region solid red. Both modes exist for
parity with Good Display's reference driver; on a colour panel neither is faster than `Full`.

Region-limited updates still work on colour panels — narrow the RAM window with
`set_window`/`set_cursor`, write **both** colour channels for that region, then refresh on
the `Full` waveform. Only the windowed area is redrawn, but it costs a full refresh. This
mirrors GxEPD2's `GxEPD2_154_Z90c`, where `partial_refresh_time == full_refresh_time` and
`hasFastPartialUpdate == false`.

For genuine sub-second differential updates, use a monochrome panel: `GDEM0213B74`
(`Ssd1680RefreshMode::Partial`), `GDEY037T03` (`Uc8253RefreshMode::FastPartial`),
`GDEQ0426T82` (`Ssd1677RefreshMode::Partial`), or the Pervasive Displays panels via
`PervasiveRefreshMode::Fast` and `write_fast_frame`.

## Quick Start

Add `epdsi` to your `Cargo.toml`:

```toml
[dependencies]
epdsi = "0.1.0"
embedded-graphics = "0.8"
```

### Cargo features

| Feature | Default | Description |
| :--- | :---: | :--- |
| `graphics` | yes | Implements `embedded-graphics-core`'s `DrawTarget` and `Dimensions` for `PageBuffer`. Disable it to drop the `embedded-graphics-core` dependency; `PageBuffer` and `render_paged` still work, you just draw into the buffer yourself. |
| `defmt` | no | Derives `defmt::Format` on the public error and mode enums (`EpdBusError`, `Spi3BusError`, `PervasiveBwryOtpError`, `ColorMode`, `ColorChannel`, `SevenColor`, and the per-controller refresh/variant enums) for logging on embedded targets. |

The minimum supported Rust version is **1.75**.

The snippets below are abridged; for complete flashable programs see [Examples on real hardware](#examples-on-real-hardware).

### 1. Usage Example (SSD1681 Controller + GDEM0154Z90 Panel)

```rust,ignore
use epdsi::prelude::*;
use embedded_graphics::{prelude::*, primitives::{Rectangle, PrimitiveStyle}, pixelcolor::BinaryColor, geometry::{Point, Size}};

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

### 3. Usage Example (PervasiveBwController + E2266KS0C1 Panel)

```rust,ignore
use epdsi::prelude::*;

// Initialize SPI bus wrapper and Pervasive Displays controller
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = PervasiveBwController::new(E2266KS0C1::WIDTH, E2266KS0C1::HEIGHT);

// Build driver for Pervasive Displays 2.66" 152x296 Monochrome panel
let mut epd = EpdBuilder::<_, E2266KS0C1>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();

// Clear RAM channel and refresh
epd.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
epd.refresh(&mut delay).unwrap();
epd.sleep(&mut delay).unwrap();
```

### 4. Usage Example (ED2208 Controller + GDEP073E01 Spectra 6 Panel)

```rust,ignore
use epdsi::prelude::*;

// Initialize SPI bus wrapper and ED2208 controller
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);

// Build driver for 7.3" 800x480 Spectra 6 EPD display (e.g. Seeed reTerminal E1002)
let mut epd = EpdBuilder::<_, GDEP073E01>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();

// Clear display frame buffer (fill with White, 0x11)
epd.clear_frame(ColorChannel::Color7(0), SevenColor::pack(SevenColor::White, SevenColor::White)).unwrap();

// Send 4bpp packed 7-color frame buffer (192,000 bytes for 800x480)
epd.write_frame(ColorChannel::Color7(0), &seven_color_frame_buf).unwrap();
epd.refresh(&mut delay).unwrap();
```

### 5. Usage Example (Ssd1680Controller + GDEM0213B74 Panel)

```rust,ignore
use epdsi::prelude::*;

// Initialize SPI bus wrapper and SSD1680(Z) controller
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Ssd1680Controller::new(GDEM0213B74::WIDTH, GDEM0213B74::HEIGHT)
    .with_refresh_mode(Ssd1680RefreshMode::Full);

// Build driver for Adafruit 6383 2.13" Monochrome display
let mut epd = EpdBuilder::<_, GDEM0213B74>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();
epd.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
epd.refresh(&mut delay).unwrap();
epd.sleep(&mut delay).unwrap();
```

### 6. Usage Example (Uc8253Controller + GDEY037T03 Panel)

```rust,ignore
use epdsi::prelude::*;

// Initialize SPI bus wrapper and UC8253 controller (note: this panel's BUSY pin is active-low)
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Uc8253Controller::new(GDEY037T03::WIDTH, GDEY037T03::HEIGHT)
    .with_refresh_mode(Uc8253RefreshMode::FastFull);

// Build driver for Adafruit 6395 3.7" Monochrome display
let mut epd = EpdBuilder::<_, GDEY037T03>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();
epd.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
epd.refresh(&mut delay).unwrap();
epd.sleep(&mut delay).unwrap();
```

### 7. Usage Example (Uc8253Controller + SE0352N14TNGA0 Panel)

```rust,ignore
use epdsi::prelude::*;

// Same UC8253 IC as the GDEY037T03 above, but a different register profile: the variant is
// not optional here. The default profile's init, RAM plane order and CDI value all differ,
// and picking it renders inverted or blank rather than erroring.
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
    .with_variant(Uc8253Variant::Se0352n14);

// Build driver for the Waveshare 3.52" e-Paper HAT (B), 240x360 Tri-Color
let mut epd = EpdBuilder::<_, SE0352N14TNGA0>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();

// 0x00 is white in BOTH planes on this panel — the opposite of the monochrome UC8253
// panel's 0xFF. Set bits are ink.
epd.clear_frame(ColorChannel::BlackWhite, 0x00).unwrap();
epd.clear_frame(ColorChannel::RedYellow, 0x00).unwrap();
epd.refresh(&mut delay).unwrap();

// sleep() enters deep sleep; call init() again before the next frame.
epd.sleep(&mut delay).unwrap();
```

### 8. Usage Example (Ssd1677Controller + GDEQ0426T82 Panel)

```rust,ignore
use epdsi::prelude::*;

// Initialize SPI bus wrapper and SSD1677 controller
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

// Build driver for Seeed Studio 6398 4.26" Monochrome display
let mut epd = EpdBuilder::<_, GDEQ0426T82>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();
epd.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
epd.refresh(&mut delay).unwrap();
epd.sleep(&mut delay).unwrap();
```

### 9. Usage Example (PervasiveBwryController + E2154QS0F1 Panel)

```rust,ignore
use epdsi::prelude::*;

// The BWRY OTP register read is a bit-banged 3-wire handshake (SCK + a single bidirectional
// DATA line), NOT the hardware SPI peripheral — the panel drives its response back on MOSI, and
// MISO is never used. `sck`/`mosi` must start as plain GPIO here (not SPI-function-bound) so
// `read_otp` can flip `mosi`'s direction; `mosi` must implement `epdsi::bus3::DynamicPin`.
let mut controller = PervasiveBwryController::new(E2154QS0F1::WIDTH, E2154QS0F1::HEIGHT)
    .with_variant(PervasiveBwryVariant::DriverF)
    .with_temperature(25);
let mut bus3 = Spi3Bus::new(cs_pin, sck_pin, mosi_pin, dc_pin, rst_pin, busy_pin);
controller.read_otp(&mut bus3, &mut delay).unwrap();
let (cs_pin, sck_pin, mosi_pin, dc_pin, rst_pin, busy_pin) = bus3.release();

// Reconfigure sck_pin/mosi_pin into the hardware SPI peripheral's function, build the SPI
// device, then wrap it with the normal 4-wire SpiBusWrapper for everything else.
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let mut epd = EpdBuilder::<_, E2154QS0F1>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();

// Send 2bpp packed BWRY frame buffer (5,776 bytes for the 152x152 panel)
epd.write_frame(ColorChannel::BlackWhite, &bwry_frame_buf).unwrap();
epd.refresh(&mut delay).unwrap();
epd.sleep(&mut delay).unwrap();
```

### 10. Usage Example (Ssd1680Controller + GDEY0266Z90 Panel)

```rust,ignore
use epdsi::prelude::*;

// Same SSD1680 profile as the monochrome GDEM0213B74 above — no variant selection needed.
let epd_bus = SpiBusWrapper::new(spi_device, dc_pin, rst_pin, busy_pin);
let controller = Ssd1680Controller::new(GDEY0266Z90::WIDTH, GDEY0266Z90::HEIGHT)
    .with_refresh_mode(Ssd168xRefreshMode::Full);

// Build driver for the Good Display GDEY0266Z90 / Waveshare 2.66" (B), 152x296 Tri-Color
let mut epd = EpdBuilder::<_, GDEY0266Z90>::new(controller).build(epd_bus);

epd.init(&mut delay).unwrap();

// The two RAM planes disagree on ink polarity: 0xFF is white in the Black/White plane, but the
// Red plane is inverted, so 0x00 is *no* red and a set bit is red. Both vendor drivers and
// GxEPD2 write `~color` for this reason.
epd.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
epd.clear_frame(ColorChannel::RedYellow, 0x00).unwrap();
epd.refresh(&mut delay).unwrap();

// sleep() enters deep sleep; call init() again before the next frame.
epd.sleep(&mut delay).unwrap();
```

## Examples on real hardware

The snippets above are `rust,ignore` because they need real SPI and GPIO. For complete,
flashable programs covering every supported controller, see:

- [`rust-rpico2-discovery`](https://github.com/melastmohican/rust-rpico2-discovery) — RP2350 Pico 2, `rp-hal`, blocking (Cortex-M33)
- [`adafruit-feather-thinkink-discovery`](https://github.com/melastmohican/adafruit-feather-thinkink-discovery) — Adafruit Feather RP2040 ThinkInk, `rp-hal` via BSP, blocking (Cortex-M0+). Panels seat directly in the board's 24-pin FPC socket, so there is no carrier or jumper wiring
- [`rust-reterminal-e1002-examples`](https://github.com/melastmohican/rust-reterminal-e1002-examples) — Seeed reTerminal E1002 (XIAO ESP32-S3), Embassy + `esp-hal`, async (Xtensa)
- [`xiao-esp32c3-blinky`](https://github.com/melastmohican/xiao-esp32c3-blinky) — Seeed XIAO ESP32-C3 on the ePaper Driver Board for XIAO, `esp-hal`, blocking (RISC-V). **Bring-up in progress**: the module used for that work was later found to be faulty, so its board-specific findings are being re-tested

| Example | Controller | Panel | Board |
| :--- | :--- | :--- | :--- |
| [`ssd1681_gdem0154z90_epd.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/ssd1681_gdem0154z90_epd.rs) | `Ssd1681Controller` | `GDEM0154Z90` — 1.54" Tri-Color | RP2350 |
| [`ssd1680_gdem0213b74_epd.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/ssd1680_gdem0213b74_epd.rs) | `Ssd1680Controller` | `GDEM0213B74` — 2.13" Mono | RP2350 |
| [`ssd1680_gdey0266z90_epd.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/ssd1680_gdey0266z90_epd.rs) | `Ssd1680Controller` | `GDEY0266Z90` — 2.66" Tri-Color | RP2350 |
| [`ssd1680_gdem0213b74_epd.rs`](https://github.com/melastmohican/adafruit-feather-thinkink-discovery/blob/main/examples/ssd1680_gdem0213b74_epd.rs) | `Ssd1680Controller` | `GDEM0213B74` — 2.13" Mono | RP2040 |
| [`ssd1680_gdey0266z90_epd.rs`](https://github.com/melastmohican/adafruit-feather-thinkink-discovery/blob/main/examples/ssd1680_gdey0266z90_epd.rs) | `Ssd1680Controller` | `GDEY0266Z90` — 2.66" Tri-Color | RP2040 |
| [`jd79661_zjy122250_epd.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/jd79661_zjy122250_epd.rs) | `Jd79661Controller` | `ZJY122250_0213AJH_E5` — 2.13" Quad-Color | RP2350 |
| [`uc8253_gdey037t03_epd.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/uc8253_gdey037t03_epd.rs) | `Uc8253Controller` | `GDEY037T03` — 3.7" Mono | RP2350 |
| [`uc8253_se0352n14_epd.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/uc8253_se0352n14_epd.rs) | `Uc8253Controller` (`Uc8253Variant::Se0352n14`) | `SE0352N14TNGA0` — 3.52" Tri-Color | RP2350 |
| [`ssd1677_gdeq0426t82_epd.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/ssd1677_gdeq0426t82_epd.rs) | `Ssd1677Controller` | `GDEQ0426T82` — 4.26" Mono | RP2350 |
| [`pdi_e2266ks0c1.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/pdi_e2266ks0c1.rs) | `PervasiveBwController` (Driver C) | `E2266KS0C1` — 2.66" Mono | RP2350 |
| [`pdi_e2290ks0f1.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/pdi_e2290ks0f1.rs) | `PervasiveBwController` (Driver F) | `E2290KS0F1` — 2.90" Mono | RP2350 |
| [`pdi_e2154qs0f1.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/pdi_e2154qs0f1.rs) | `PervasiveBwryController` (Driver F) | `E2154QS0F1` — 1.54" Spectra-4 | RP2350 |
| [`pdi_e2417qs0a3.rs`](https://github.com/melastmohican/rust-rpico2-discovery/blob/main/examples/pdi_e2417qs0a3.rs) | `PervasiveBwryController` (Driver A) | `E2417QS0A3` — 4.2" Spectra-4 | RP2350 |
| [`epd_ed2208_demo.rs`](https://github.com/melastmohican/rust-reterminal-e1002-examples/blob/main/examples/epd_ed2208_demo.rs) | `Ed2208Controller` | `GDEP073E01` — 7.3" Spectra 6 | ESP32-S3 |
| [`epd_ed2208_bmp.rs`](https://github.com/melastmohican/rust-reterminal-e1002-examples/blob/main/examples/epd_ed2208_bmp.rs) | `Ed2208Controller` | `GDEP073E01` — 7.3" Spectra 6, BMP rendering | ESP32-S3 |

Every supported controller has a working example, verified on hardware.

The table lists at least one example per controller; the sibling repositories port several of them
to other hosts, keeping everything above `main()` byte-identical to the RP2350 originals so that
only board bring-up differs. That is the point of the `EpdPanel` / `EpdController` /
`SpiBusWrapper` split: the same driver code runs unchanged across **Cortex-M0+, Cortex-M33, RISC-V
and Xtensa**, under two HAL families and both blocking and async executors.

That portability is also a measurement, not just a claim. The `GDEM0213B74` on an SSD1680 gives a
3894 ms full refresh and 1018 ms differential partial on RP2350, and 3893 / 1017 ms on a Feather
RP2040 — the same numbers to within a millisecond, from identical driver code on two MCU families.
Running one diagnostic across hosts is also how a failing board gets identified rather than
mistaken for a driver defect; see [Troubleshooting](#3-a-different-microcontroller).

## Troubleshooting on real hardware

Every controller here is register-level parity with a vendor reference driver, and the tests assert
exact SPI byte streams. So when a panel misbehaves, the register sequence is rarely the cause —
across this project's bring-ups it has almost always been one of four things, and they are worth
working in this order:

1. **Panel state** — the panel is not in the condition you think it is.
2. **Identity** — the panel is not the panel you think it is.
3. **The board** — timing and power differ between MCUs even though the driver code does not.
4. **The glass** — the waveform lives in the panel, and varies by supplier and batch.

### 0. Power-cycle before you debug anything

E-paper retains its last write, and the controller can be left latched busy by an interrupted run
or a hot-swapped FPC. The *next* run then hits busy timeouts and looks broken — shifted content,
refreshes returning instantly, refreshes that appear to hang. `hard_reset` does not clear it; only
removing power does. Connect and disconnect FPCs with the board unpowered.

**Time your refreshes.** It is the cheapest diagnostic there is, and it distinguishes "slow" from
"never ran": a tri-colour panel physically cannot update in 100 ms, so a refresh that returns that
fast did not drive the display. Equally, a refresh reported as 100 ms when the panel demonstrably
takes 3.9 s means your instrument is broken, not the driver.

Reason from a clean run only. Never from an interrupted one, or from any run after one.

### 1. Is it the panel you think it is?

The same glass ships behind different controllers. DKE's 2.66" family is the cautionary case — all
152 × 296, all 24-pin, visually identical:

| Part number | Driver IC |
| :--- | :--- |
| `DEPG0266RW`**`S800`**`F34HP` | SSD1680 — works with `Ssd1680Controller` |
| `DEPG0266RW`**`F51B`**`F1` | JD79651B — **not supported by that controller** |
| `DEPG0266RW`**`U25D`**`F15` | UC8251d — **not supported by that controller** |

Read the label before assuming a driver fault. [CursedHardware/epd-datasheet](https://github.com/CursedHardware/epd-datasheet/blob/main/epd-display.csv)
maps part numbers to driver ICs for most vendors, and grepping a part-number stem reveals the
vendor's own encoding by comparison.

The second identity trap is internal: `EpdController` never sees the `PANEL` type, so a
**controller/panel variant mismatch cannot be caught at compile time**. `Ssd168xVariant`,
`Uc8253Variant`, `PervasiveDriverVariant` and `PervasiveBwryVariant` all render inverted or blank
rather than returning an error. If a panel needs a non-default variant, its module docs say so.

### 2. Symptom lookup

| Symptom | Likely cause |
| :--- | :--- |
| Nothing at all; BUSY never releases | Wiring or power. Check `busy_active_high` matches the panel — several panels here are active-**low**. On EXT3-1 boards with panels ≤ 3.7", the **J3 jumper must be OPEN** |
| Refresh returns in milliseconds | Controller latched busy from an earlier interrupted run — power-cycle (see above) |
| Whole panel comes up black, or stays white | Wrong `clear_frame` fill byte for that plane's ink polarity |
| Colour panel: a region or the whole screen is solid red/yellow | Colour-plane polarity. On SSD168x tri-colour, `0x24` wants `0xFF` for white but `0x26` wants `0x00` for *no* colour — the planes disagree |
| Image mirrored or sheared | `WIDTH`/`HEIGHT` transposed. `WIDTH` is the **short** axis; vendors advertise the landscape figure |
| Image correct but rotated 180° | Mounting or connector orientation, not a driver fault — use `DisplayRotation::Rotate180` |
| Content shifted a few pixels per row | Row stride. A width that is not a byte multiple still occupies `width.div_ceil(8)` bytes |
| Colour vanishes on a partial update | `Partial` on a colour panel — the fast LUT is monochrome-only and drops the colour plane |
| Correct geometry, weak colour or ghosting | The glass's waveform, not the registers — see §4 |

### 3. A different microcontroller

The same driver code runs across Cortex-M0+, Cortex-M33, RISC-V and Xtensa. When a panel works on
one board and not another, the difference has consistently been **timing, power, or the board
itself — not logic**. Real examples from this project, all found on one ESP32-C3 and none of which
reproduced on RP2350:

- The controller dropped its charge pump after an update, so the next bare refresh was silently
  ignored — BUSY never asserted, the poll read idle, and `refresh` returned in 0 ms having drawn
  nothing. Fixed by issuing `POWER_ON` before every refresh.
- BUSY was not asserted instantly, and no fixed settling delay could be tuned to cover it — 10 ms
  held on some refreshes and missed others in the same run. Fixed by waiting for the BUSY edge
  (`SpiBusWrapper::wait_busy_assert`), bounded so a missing panel still reads idle rather than
  hanging.
- The reset pulse was 2 ms, matching the vendor driver, and latched only intermittently. Now 30 ms.

**That module was later found to be faulty**, by substitution: a different MCU ran clean on the
same carrier, cable and panel. All three changes are kept because each is independently justified
by a vendor reference rather than only by those symptoms — but the episode is the lesson. Symptoms
chased on a single board can be the board.

So the method matters more than the list above: **run one identical diagnostic on a second host.**
If the code is the same and only the host differs, a discrepancy localises to the host. Worked
example, all on one 2.13" panel and one driver build:

| Host | Full refresh | Differential partial |
| :--- | ---: | ---: |
| RP2350 | 3894 ms | 1018 ms |
| Feather RP2040 | 3893 ms | 1017 ms |
| ESP32-C3, healthy | ~3891 ms | ~1017 ms |
| ESP32-C3, faulty module | 7450 ms | 98 ms |

Two healthy hosts agreeing to a millisecond, and one host deviating in *both* directions at once,
is not a driver result. Note the shape of the bad row: too slow **and** too fast. A uniform stretch
would suggest a slow clock; deviation in both directions says the waveform is not executing as
specified, which is electrical.

So: suspect supply decoupling, cables and connectors, SPI clock, reset timing, and the board
itself, before suspecting registers.

### 4. A different panel batch or glass vendor

Waveshare, Good Display and DKE sell the same panel, and a module may ship with glass from any of
them — the `GDEY0266Z90` supported here was brought up on DKE glass stamped `DEPG0266RWS800F34HP`.
Electrically that is fine, and it is checkable rather than assumed: GxEPD2's DKE driver
(`GxEPD2_266_BN`) and its Good Display driver (`GxEPD2_266c`) have identical init register sets.

What *does* differ is the **OTP waveform**, which lives in the panel and is not selected by any
code here. Measured on a `GDEY0266Z90`, `Ssd168xRefreshMode::FastFull` took **16.2 s against 20.0 s**
for `Full` — a real 19 % saving — where Good Display quote only ~19 s against ~20 s on their own
glass. Same IC, same resolution, different glass.

Two consequences worth internalising:

- **Every timing figure in these docs is a reference point, not a guarantee.** Measure on the panel
  in front of you. The `ssd1680_gdey0266z90_epd` example logs its own refresh durations for exactly
  this reason.
- **Weak or pink colour, ghosting, uneven ink density and a different refresh duration are waveform
  symptoms, not register faults.** No amount of register auditing fixes them, and nothing in `epdsi`
  can select a different waveform.

### Debugging discipline

Distilled from the bring-ups behind this crate, in rough order of how much time each has saved:

- **Suspect hardware and panel state before software.** Running stock Arduino GxEPD2 across a
  couple of boards has twice found in one experiment what hours of driver analysis did not.
- **Validate any new diagnostic against a known measurement** before trusting what it tells you.
- **Watch the panel, not the log.** A hand-rolled trigger sequence once reported entirely plausible
  timings while never driving the display at all.
- **Change one thing at a time**, and power-cycle between attempts.
- **Reproduce on a second host before concluding anything about the driver** — see §3.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

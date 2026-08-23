# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.1] - 2026-08-23

Documentation and discoverability. No API or behaviour changes — every panel and
controller works exactly as in 0.1.0.

### Added

- Substantial crate-level documentation. `docs.rs/epdsi` previously showed three
  sentences; it now covers the architecture, the supported controller/panel table with
  links into the API, a quick-start example, paged rendering, why colour panels cannot
  refresh quickly, the cargo features, and the EXT3-1 J3 jumper hardware note.
- README banner showing all ten supported panels running on real hardware, plus a CI
  status badge.
- README section linking complete, flashable examples for every supported controller,
  in [`rust-rpico2-discovery`](https://github.com/melastmohican/rust-rpico2-discovery)
  (RP2350, `rp-hal`) and
  [`rust-reterminal-e1002-examples`](https://github.com/melastmohican/rust-reterminal-e1002-examples)
  (ESP32-S3, Embassy + `esp-hal`).
- README table documenting the `graphics` and `defmt` cargo features, and the MSRV.

### Fixed

- The quick-start example used `PrimitiveStyle` without importing it, so copy-pasting it
  produced a compile error. Fixed in both the crate docs and the README.

### Changed

- The crate description now names the supported driver ICs, and `e-ink` and
  `embedded-graphics` replace `display` and `no-std` in the keywords, so the crate is
  findable by searching for a specific controller. (`no-std` remains a category.)
- The quick-start example is now a compiled `no_run` doctest rather than `ignore`, so it
  cannot silently drift from the API, and CI runs rustdoc with `-D warnings` so broken
  intra-doc links fail the build.
- The docs badge points at `img.shields.io/docsrs/epdsi` directly instead of redirecting
  through `docs.rs/epdsi/badge.svg`.

## [0.1.0] - 2026-08-22

Initial release.

### Added

- Modular driver framework separating panel specs (`EpdPanel`), driver IC register
  logic (`EpdController`), and physical transport (`SpiBusWrapper`), tied together by
  `EpdDriver` and constructed via `EpdBuilder`.
- `embedded-hal` 1.0 transport built on `SpiDevice`, `OutputPin`, `InputPin`, and
  `DelayNs`, with configurable BUSY polarity and both spin-loop and delay-based
  busy-polling.
- `Spi3Bus`, a bit-banged 3-wire SPI bus used to read OTP registers from Pervasive
  Displays BWRY COGs.
- Unified color model (`ColorMode`, `ColorChannel`, `SevenColor`) covering Monochrome,
  Tri-Color (B/W/Red), Quad-Color (2 bpp), and 7-Color ACeP (4 bpp) panels.
- Low-RAM paged rendering: `PageBuffer` plus the `render_paged` sweep helper, which
  keeps frame memory to a single stack-allocated horizontal page.
- `embedded-graphics` integration via `DrawTarget`/`Dimensions` behind the default
  `graphics` feature.
- Controller support for SSD1680, SSD1681 (both via `Ssd168xController`, with
  `Ssd1680Controller`/`Ssd1681Controller` as distinct types), SSD1677, UC8253, ED2208,
  JD79661, and the Pervasive Displays COG families (`PervasiveBwController` for
  Driver C/F, `PervasiveBwryController` for Driver A/F).
- Panel support for `GDEM0154Z90`, `GDEM0213B74`, `ZJY122250_0213AJH_E5`
  (`GDEY0213F51`), `GDEY037T03`, `GDEQ0426T82`, `GDEP073E01`, `E2266KS0C1`,
  `E2290KS0F1`, `E2154QS0F1`, and `E2417QS0A3`, with GxEPD2- and Pervasive-style type
  aliases for vendor naming parity.
- Automatic RAM alignment of panel widths to hardware byte boundaries, so panels whose
  width is not a multiple of 8 (such as the 122 px `GDEM0213B74`) address the correct
  number of bytes per row.
- Optional `defmt` feature deriving `defmt::Format` on the public error and mode enums.
- `no_std` builds verified against `thumbv6m-none-eabi`, `thumbv7em-none-eabihf`, and
  `riscv32imac-unknown-none-elf`.

[Unreleased]: https://github.com/melastmohican/epdsi/compare/v0.1.1...HEAD
[0.1.1]: https://github.com/melastmohican/epdsi/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/melastmohican/epdsi/releases/tag/v0.1.0

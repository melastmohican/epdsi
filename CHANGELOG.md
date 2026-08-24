# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.3] - 2026-08-23

### Added

- `SpiBusWrapper::busy_is_high`, exposing the current level of the BUSY pin. Panels could
  previously only be *waited on*, never observed, so there was no way to tell a slow
  refresh from a stalled one, or to time a refresh at all. That gap turned a hardware
  fault into a long hunt during bring-up on a XIAO ESP32-C6, where corrupted SPI meant the
  panel never acted on commands and `refresh` returned instantly — indistinguishable from
  a driver bug without a way to sample BUSY.

### Fixed

- Two rows of the hardware-examples table still described `GDEP073E01` as a 7.3" ACeP
  panel. It is E Ink Spectra 6; 0.1.2 corrected this elsewhere but missed these.

### Changed

- README lists [`xiao-esp32c3-blinky`](https://github.com/melastmohican/xiao-esp32c3-blinky)
  among the hardware examples — four panels on a XIAO ESP32-C3, and the first RISC-V host
  with verified hardware. With the existing RP2350 (Cortex-M33) and reTerminal E1002
  (Xtensa) repositories, the same driver code is now exercised unchanged across three
  architectures, two HAL families, and both blocking and async executors.
- `ZJY122250_0213AJH_E5` documents the `FPC-J002` flex ribbon stamp as an identification
  aid. The same panel ships under Good Display, Seeed and Adafruit part numbers with
  different stickers; units from different vendors are physically identical and carry the
  same ribbon, so the stamp identifies the panel where the retail labelling does not.

## [0.1.2] - 2026-08-23

Documentation correction. No API or behaviour changes.

### Fixed

- `GDEP073E01` was documented throughout as a "7-Color ACeP" panel. It is an **E Ink
  Spectra 6** panel — vendor part `GDEP073E01(E6)` — rendering six colours: black,
  white, red, yellow, blue and green. Corrected in the panel and controller docs and
  in the crate-level and README tables.
- `SevenColor::Orange` is **not renderable** on Spectra 6 panels, including
  `GDEP073E01`; it belongs to the older ACeP 7-colour generation and previously
  produced an undefined colour with nothing documenting why. The variant now carries
  that warning, and `SevenColor::Clean` is documented as rendering white.

`SevenColor` keeps its name and discriminants — the values are the panels' native
codes and are correct, and the palette spans both the ACeP 7-colour and Spectra 6
generations.

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

[Unreleased]: https://github.com/melastmohican/epdsi/compare/v0.1.3...HEAD
[0.1.3]: https://github.com/melastmohican/epdsi/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/melastmohican/epdsi/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/melastmohican/epdsi/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/melastmohican/epdsi/releases/tag/v0.1.0

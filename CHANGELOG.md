# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- `Ssd1677Controller` clears a colour plane with the controller's own RAM pattern generator
  (`0x46` / `0x47`) instead of streaming every byte. A full-plane clear on an 800 × 480
  `GDEQ0426T82` now costs one command and one data byte rather than 48,000 streamed bytes.

  These registers drive a *regular pattern*, not a memset — `A[7]` is one step's value, `A[6:4]`
  the step height in gates (max 680) and `A[2:0]` the step width in sources (max 960) — so the
  fast path is taken only where the result is provably uniform: a `0x00` or `0xFF` fill, covering
  a whole plane, on a panel inside 960 × 680. `0xF7` and `0x77` are consequently the only two
  bytes ever sent. Everything else streams exactly as before, including partial fills and
  non-uniform bytes. BUSY is waited on, since the sweep runs in hardware and leaves the RAM
  address counter where it finished.

  **No vendor reference driver uses these registers on this panel.** Neither
  `GxEPD2_426_GDEQ0426T82` nor Good Display's own `GDEY0426T82` sample does, and
  `GxEPD2_370_TC1` carries both commented out and marked "DON'T USE WITH GxEPD2" — a note about
  GxEPD2's shadow-buffer bookkeeping, which a full-RAM sweep desynchronises, rather than a defect
  in the controller. `epdsi` keeps no such buffer. The semantics here come from the SSD1677
  datasheet (Rev 1.0, Nov 2018) directly, which makes this the one part of the driver with no
  reference implementation behind it. If a cleared panel comes up banded or half-inverted,
  `with_ram_auto_fill(false)` restores 0.1.6 behaviour byte for byte and is the first thing to
  try.

### Added

- `Ssd1677Controller::with_ram_auto_fill` and `ram_auto_fill`, controlling the above. Enabled by
  default.

## [0.1.6] - 2026-09-01

### Added

- `EpdPanel::VCOM`, `EpdPanel::CUSTOM_LUT` and `EpdPanel::GATE_VOLTAGE` — defaulted associated
  consts replacing the `vcom()`, `custom_lut()` and `gate_voltage()` methods. The methods were
  unreachable by construction: panels are zero-sized types held through `PhantomData`, so no
  instance ever existed to call them on, and no panel-declared register override could reach a
  controller. The consts can.
- `Ssd1680Controller::for_panel::<P>()`, `Ssd1681Controller::for_panel::<P>()`,
  `Ssd168xController::for_panel::<P>(variant)` and `Ssd1677Controller::for_panel::<P>()`, which
  read a panel's dimensions *and* its register configuration off `EpdPanel`. These collapse the
  `new(P::WIDTH, P::HEIGHT)` pairing every example repeats.
- `with_vcom`, `with_gate_voltage` and `with_lut` builders (plus matching getters) on the SSD168x
  and SSD1677 controllers — the two ICs that actually have `0x2C` / `0x03` / `0x32` registers.
  Configured values are written during `init_sequence` in the order
  `GxEPD2_213_B72::_InitDisplay()` uses: VCOM then gate voltage straight after the border
  waveform, and the LUT last, after the RAM window and cursor, per `_Init_Full()`.

  Deliberately **not** added to UC8253, JD79661, ED2208 or the Pervasive pair. Their
  configuration has a different shape, and the BWRY panel's comes from OTP at runtime rather than
  from a panel const.

### Changed

- Adopted a `[lints.clippy]` policy denying `unwrap_used`, `expect_used`, `panic`, `todo` and
  `unimplemented` in library code. `src/` was already clean, so nothing needed rewriting.

### Removed

- `GDEM0154Z90` no longer declares a VCOM override. It previously carried `vcom() -> Some(0x26)`,
  which never left the crate because the hook was unreachable — and which is not this panel's
  value: `GxEPD2_154_Z90c::_InitDisplay()` writes no `0x2C` at all, running on the panel's OTP
  VCOM. `0x26` is what `GxEPD2_213_B72::_Init_Part()` writes for a different panel on a different
  IC in partial mode. Promoting it to the new const, now that the const reaches the wire, would
  have converted a dead placeholder into a live divergence.

  **No byte on any wire changed.** Every panel `epdsi` ships declares no override, so
  `for_panel::<P>()` produces a byte-identical init to `new(P::WIDTH, P::HEIGHT)` for all of them
  — asserted in `tests/ssd1680_tests.rs`, `tests/ssd1681_tests.rs` and `tests/ssd1677_tests.rs`.

### Deprecated

- `EpdPanel::vcom()`, `EpdPanel::custom_lut()` and `EpdPanel::gate_voltage()`. Use the
  `VCOM`, `CUSTOM_LUT` and `GATE_VOLTAGE` consts instead. Scheduled for removal in 0.2.0.

### Fixed

- Documentation: the `wait_busy_with_delay` cap is a 60,000 ms safety timeout, not 1500
  iterations.

## [0.1.5] - 2026-08-30

### Added

- `GDEY0266Z90`, the 152 × 296 Tri-Color panel sold by Good Display under that name and by
  Waveshare as the 2.66" e-Paper Module (B), with the GxEPD2 parity alias `GxEPD2_266c`. It needs no
  controller variant: a register audit against `GxEPD2_266c` and the Waveshare and Good Display
  reference drivers found the existing `Ssd168xVariant::Ssd1680` profile already drives it
  byte-for-byte — same `0x01` gate count, `0x11` data entry, `0x3C` border, `0x18`/`0x21` control
  bytes, RAM window arithmetic and `0xF7` refresh.

  The one thing that will silently mis-render is ink polarity, because the two RAM planes disagree.
  `0xFF` is white in the Black/White plane (`0x24`), but the Red plane (`0x26`) is **inverted**:
  `0x00` is no red and a *set* bit is red. A white panel is therefore
  `clear_frame(ColorChannel::BlackWhite, 0xFF)` plus `clear_frame(ColorChannel::RedYellow, 0x00)` —
  the same asymmetry as the `GDEM0154Z90`, and the opposite of the `SE0352N14TNGA0`, which clears
  both planes to `0x00`. All three C++ references write `~color` for this reason.

  Verified on hardware: RP2350 Pico 2 over a Good Display DESPI-C02, all four refresh modes, both
  planes rendering with correct polarity and orientation.
- `Ssd168xRefreshMode::FastFull` and `Ssd168xRefreshMode::BaseMap`, ported from Good Display's
  `GDEY0266Z90` reference driver — GxEPD2 has no counterpart, since `GxEPD2_266c` drives both its
  full and its "partial" refresh on `0xF7`.

  `FastFull` (`0xC7`) is preceded by the vendor's temperature override: load the sensor reading
  (`0x22 0xB1`), write 90 °C into the temperature register (`0x1A 0x5A 0x00`), then reload the OTP
  LUT at that temperature (`0x22 0x91`). Good Display issue this from a dedicated
  `EPD_HW_Init_Fast()` that skips driver output control, data entry mode and the RAM window
  entirely, leaving the fast pass on power-on defaults; `epdsi` issues it from `trigger_refresh`
  instead, so the window set up by `init_sequence` always applies. This matches how
  `Ssd1677RefreshMode::FastFull` already handles its own temperature override. How much it saves
  depends on the panel's OTP waveform rather than the controller: measured at 16.2 s against 20.0 s
  for `Full` on a `GDEY0266Z90` (DKE glass, a 19 % saving), where Good Display quote only ~19 s
  against ~20 s for their own glass. No colour panel reaches the sub-second figures a monochrome
  SSD168x panel does in this mode, because the red pigment has no differential waveform to skip.

  `BaseMap` (`0xF4`) primes the controller's previous-frame buffer before a run of `Partial`
  updates, which is a **monochrome-only** workflow. On a colour panel `0x26` is always the colour
  plane, so seeding it with a Black/White image — correct on the `GDEM0213B74` — sets nearly every
  bit and renders the region solid red. Measured on a `GDEY0266Z90`, `BaseMap` and `Partial` both
  take ~19.9 s, indistinguishable from `Full`; they exist for parity with the reference driver.

  `Full` and `Partial` keep their existing bytes and power envelope, so `GDEM0213B74` and
  `GDEM0154Z90` are unaffected. Adding enum variants is source-breaking for downstream code that
  matches `Ssd168xRefreshMode` exhaustively.

### Changed

- README gains a **Troubleshooting on real hardware** section. Every failure this project has hit
  on hardware has been outside the register sequence — panel state, panel identity, the host board,
  or the glass's waveform — and none of that was documented anywhere a `crates.io` user could reach
  it. Covers power-cycle discipline, a symptom-to-cause table, decoding a part number to its driver
  IC (the same 2.66" glass ships behind an SSD1680, a JD79651B or a UC8251d), and why timing figures
  in these docs are reference points rather than guarantees.
- README documents the cross-host method: run one identical diagnostic on a second host, because a
  discrepancy then localises to the host rather than the driver. Includes the worked example that
  identified a faulty ESP32-C3 module — two healthy hosts agreeing to a millisecond, one deviating
  in both directions at once.
- `adafruit-feather-thinkink-discovery` added to the hardware examples: Feather RP2040 ThinkInk,
  with panels seated directly in the board's FPC socket. `GDEM0213B74` and `GDEY0266Z90` both
  verified there, giving a fourth MCU family (Cortex-M0+) and a second independent host for the
  SSD1680 timings — 3893 ms full and 1017 ms differential partial, against RP2350's 3894 / 1018.
- `xiao-esp32c3-blinky` marked as bring-up in progress. The module used for that work was later
  shown by substitution to be faulty, so its board-specific findings — including which panels do
  and do not work on that host — are being re-tested. The three UC8253 fixes in 0.1.4 were made
  chasing symptoms on it; all are kept, since each is independently justified by a vendor reference
  rather than only by those symptoms.

## [0.1.4] - 2026-08-29

### Added

- `SE0352N14TNGA0`, the 240 × 360 Tri-Color panel in the Waveshare 3.52" e-Paper HAT (B).
- `Uc8253Variant`, selecting the UC8253 register profile. The IC is shared with the existing
  `GDEY037T03`, but the two panels are not interchangeable behind one profile: the 3.52" needs an
  explicit `RESOLUTION`/`BOOSTER_SOFT_START` init, puts Black/White on the *other* RAM plane
  (`0x10`, not `0x13`), and must not have `CDI` re-issued at refresh time — `0x97` would move the
  DDX polarity bits away from the `0x87` set at init and invert black and white. Picking the wrong
  variant renders inverted or blank rather than erroring, so it has to be named:
  `Uc8253Controller::new(…).with_variant(Uc8253Variant::Se0352n14)`.

  `Uc8253Controller::new` still defaults to the `GDEY037T03` profile, so existing code is
  unaffected. Register sequences follow Waveshare's `3in52_e-Paper_B` reference driver and its
  Adafruit_EPD port; there is no GxEPD2 driver for this panel to audit against.

  Note that on this panel `0x00` is white in *both* RAM planes — the opposite of the monochrome
  UC8253 panel — so both channels clear with `clear_frame(channel, 0x00)`.
- `SpiBusWrapper::wait_busy_assert`, waiting for BUSY to assert with a bounded timeout and
  reporting whether it was observed. A panel that never asserts returns `false` rather than
  erroring or hanging, so "a missing panel reads idle" still holds.

### Fixed

- `Uc8253Variant::Se0352n14` now issues `POWER_ON` before each `DISPLAY_REFRESH`. The controller
  drops its charge pump after an update, so a bare `DISPLAY_REFRESH` on the next frame was silently
  ignored: BUSY never asserted, the poll read idle, and `refresh` returned in **0 ms** having drawn
  nothing. Waveshare's reference avoids this by re-running its entire init — which begins with
  `POWER_ON` — before every display operation, one refresh per init; the original port modelled the
  power as staying up between frames, which was wrong.
- Both UC8253 variants now wait for BUSY to *assert* after `DISPLAY_REFRESH` before waiting for it
  to clear. Polling for completion too early reads "idle" and reports a refresh that is still
  running; anything written next lands in controller RAM mid-update and the panel stays permanently
  one frame behind, rendering the current frame as streaked noise. A fixed settling delay could not
  be made reliable — a 10 ms guard held on some refreshes and missed others in the same run — so
  the wait is for the edge, bounded by a timeout. It costs one poll on hardware that behaves.

  This changes timing only, not the command stream, and the `GDEY037T03` fast-partial path is
  unaffected: only an already-broken panel pays the timeout.
- `Uc8253Variant::Se0352n14` holds RST low for 30 ms during init, matching Pervasive Displays'
  reference driver for this panel family. Waveshare's 2 ms proved marginal: the reset latched only
  intermittently, and a reset that does not take leaves the controller ignoring `POWER_ON` and
  `DISPLAY_REFRESH` alike, failing at a random frame each run.

  All three faults reproduce on a XIAO ESP32-C3 and none on an RP2350, which polls late enough and
  resets long enough to hide them.

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

[Unreleased]: https://github.com/melastmohican/epdsi/compare/v0.1.6...HEAD
[0.1.6]: https://github.com/melastmohican/epdsi/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/melastmohican/epdsi/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/melastmohican/epdsi/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/melastmohican/epdsi/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/melastmohican/epdsi/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/melastmohican/epdsi/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/melastmohican/epdsi/releases/tag/v0.1.0

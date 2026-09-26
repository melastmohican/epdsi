# Changelog

All notable changes to this project are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `EpdDriver::display_frame(channel, data, delay)`, bundling `write_frame` + `refresh` into one
  call for the common single-channel "just show this frame" case. Purely additive — existing
  `write_frame`/`refresh` primitives are unchanged and remain the right choice for multi-channel
  (Tri-Color/Quad-Color) writes or paged/partial updates.

## [0.4.2] - 2026-09-19

### Fixed

- `Jd79661Controller`'s init sequence sent `TCON` (`0x60`) with 3 data bytes (`[0x02, 0x02, 0x02]`);
  both `Adafruit_JD79661.cpp:20` (`0x60, 2, 0x02, 0x02`) and `GxEPD2_213c_GDEY0213F51.cpp:946-948`
  agree TCON takes exactly 2. Trimmed to `[0x02, 0x02]`. Low-severity in practice — the panel this
  drives (`ZJY122250_0213AJH_E5`) is hardware-verified working correctly across all four of the
  crate's blocking and async hardware hosts with the extra byte present, evidently latched
  harmlessly — but it's a real byte-level parity gap against two independent vendor references,
  fixed for the crate's own stated bar.

### Changed

- Swapped the `e-paper` keyword for `epaper` in `Cargo.toml`. crates.io's `e-paper` keyword bucket
  only reaches 2 crates; `epaper` (no hyphen) is the one nearly every peer crate — `epd-waveshare`,
  `ssd1680`, `ssd1681`, `il0373`, `epd-datafuri`, `gdep073e01`, `ssd1677-driver`, and others — is
  actually tagged with, and where real searches land.

## [0.4.1] - 2026-09-18

### Fixed

- `EpdDriver::clear_frame` computed byte counts by matching only `ColorMode::QuadColor`, so every
  other mode — including `ColorMode::SevenColor` (4bpp, 2 pixels/byte) — fell through to the
  1-bit-per-pixel formula and sent roughly 1/4 the bytes the plane needs. On `GDEP073E01`
  (800×480, the crate's only `SevenColor` panel), a full-panel clear only zeroed the top ~120 of
  480 rows — the bottom 75% kept whatever was in RAM before, showing old image data at the next
  refresh. Added a `SevenColor` arm mirroring `required_bytes()`'s existing `ColorChannel::Color7`
  handling. Hardware-verified on `rust-reterminal-e1002-examples` (`GDEP073E01`): a full-panel
  image now clears with no remnants anywhere on the panel.
- `Ed2208Controller`'s narrowed RAM window (`PARTIAL_WINDOW`/`0x83`, set via
  `EpdDriver::set_window`) never widened back out — a subsequent full-panel `write_frame`/
  `refresh()` stayed silently scoped to the last narrowed region instead of updating the whole
  panel. `trigger_refresh` now reasserts the full-panel window before every refresh, matching
  `GxEPD2_730c_GDEP073E01.cpp::refresh(bool)`. An initial attempt also reasserted the window from
  `init_sequence`, which `_InitDisplay()` never does in the reference driver; sending it there
  corrupted the panel's RAM-write state on real hardware (visible as full-panel static), caught
  and reverted before release. Hardware-verified on `rust-reterminal-e1002-examples`
  (`GDEP073E01`).

### Changed

- README: point owners of stock Waveshare modules at
  [`epd-waveshare`](https://crates.io/crates/epd-waveshare)'s panel list — `epdsi`'s panels are
  mostly Good Display/Pervasive/WeAct/Adafruit/Seeed glass with only size-class overlap against
  Waveshare's stock SKUs, no exact part-number match.

## [0.4.0] - 2026-09-13

### Added

- **`GDEM0154F51H`** (**`GxEPD2_154c_GDEM0154F51H`**), the Good Display / Waveshare 1.54inch
  e-Paper (G) module (SKU 30441, JD79660AA, 200×200, Black/White/Red/Yellow) — a new
  `Jd79660Controller`. Register sequence derived from Waveshare's `EPD_1in54g.c`/`.h` and GxEPD2's
  `GxEPD2_154c_GDEM0154F51H.cpp`/`.h`, which agree byte-for-byte on the plain init/refresh/sleep
  path, then cross-checked directly against the JD79660A datasheet (v1.0.3) for register lengths
  and defaults. Fast-update mode is not implemented — the two reference sources disagree on
  register ordering for it, and the vendor's own demo defaults to the plain path anyway.
  **Hardware-verified** blocking on RP2350 (`rust-rpico2-discovery`), RP2040
  (`adafruit-feather-thinkink-discovery`) and ESP32-C3 (`xiao-esp32c3-blinky`), and async on
  RP2350 (`rust-rpico2-embassy-examples`): init/write/refresh/sleep complete cleanly on every host,
  ~19.7s measured full refresh on RP2040 against the panel's quoted ~20s spec.

### Changed

- `Jd79661Controller` is now a thin wrapper over a shared `Jd7966xController`/`Jd7966xVariant`
  (mirroring `Ssd1680Controller`/`Ssd1681Controller` over `Ssd168xController`/`Ssd168xVariant`),
  since the JD79660A and JD79661AA datasheets share an identical SPI command-register table.
  `Jd79661Controller::new`'s public signature and behavior are unchanged apart from the fix below;
  the module moved from `src/controllers/jd79661.rs` to `src/controllers/jd7966x.rs`.

### Fixed

- `Jd79661Controller::trigger_refresh`/`sleep` now send the mandatory trailing `0x00` data byte on
  `DISPLAY_REFRESH` (`0x12`) and `POWER_OFF` (`0x02`) — previously sent bare. Both the JD79660A and
  JD79661AA datasheets document one data byte (default `0x00`) for each command, and Adafruit's own
  `Adafruit_JD79661.cpp` sends it explicitly; only reading the datasheet directly (not just the two
  vendor C++ references, which happened to agree with each other) surfaced this. Re-verified on
  the same four hardware-verified hosts as `GDEM0154F51H` above via the existing
  `ZJY122250_0213AJH_E5` examples — no regression from the byte fix.
- `EpdDriver::clear_frame`/`required_bytes` (`src/driver.rs`) computed byte counts with
  1-bit-per-pixel math for every `ColorChannel` except `Color7`, silently under-computing every
  `ColorMode::QuadColor` panel's real RAM size. Added a `QuadColor`-aware branch driven by a new
  `EpdPanel::RAM_WIDTH` const (defaulting to `WIDTH`, overridden by `ZJY122250_0213AJH_E5` to its
  documented 128px RAM-padded width). Retroactively corrects `ZJY122250_0213AJH_E5`'s `clear_frame`
  byte count from 4000 to the correct 8000.

## [0.3.1] - 2026-09-12

### Fixed

- `GDEY0266T90`'s Gray4 LUT (`GRAY4_LUT`) was declared `static` and referenced from the `const
  GRAY4` panel default — legal on newer Rust, but `error[E0013]: constants cannot refer to
  statics` on this crate's MSRV (1.75), where that restriction hadn't yet been relaxed. This broke
  **every** consumer building on the MSRV toolchain, not an edge case: `0.3.0` failed to compile at
  all under `cargo +1.75.0 check`. Caught by CI's MSRV job immediately after the `0.3.0` push (not
  before it — the regular verification matrix run pre-release used a newer local toolchain, which
  compiles the old code fine, so this slipped through). Changed `GRAY4_LUT` to `const`, verified
  against `cargo +1.75.0 check --all-features` and `--no-default-features --features graphics`
  directly before this release, not just on CI. If you're on `0.3.0`, upgrade to this release
  rather than staying pinned to it.

## [0.3.0] - 2026-09-12

### Added

- `GDEY0266T90` (`GxEPD2_266_GDEY0266T90`), the Good Display / Waveshare 2.66" **monochrome**
  e-Paper module (SSD1680, 152×296) — a different glass from the existing Tri-Color `GDEY0266Z90`
  of the same nominal size, not a config of it. Purely additive: no controller changes, no
  `VCOM`/`GATE_VOLTAGE`/`CUSTOM_LUT` override, register-identical init to `GDEY0266Z90` on the
  default SSD1680 profile — verified against the GxEPD2 reference driver
  (`GxEPD2_266_GDEY0266T90.cpp`) byte-for-byte. Unlike its Tri-Color sibling, this panel supports a
  genuine fast partial refresh (`Ssd168xRefreshMode::Partial`, ~500 ms per the reference), not the
  parity-only no-op partial mode colour panels get. **Hardware-verified** blocking on RP2350
  (`rust-rpico2-discovery`), RP2040 (`adafruit-feather-thinkink-discovery`) and ESP32-C3
  (`xiao-esp32c3-blinky`), and async on RP2350 (`rust-rpico2-embassy-examples`): `Full` and
  `Partial` both render correctly on every host, though `Partial` measured ~4.1 s per update on the
  original RP2350 unit — not the sub-second figure GxEPD2 quotes, so treat that reference number as
  panel/glass-dependent rather than assumed.

- 4-level grayscale (Gray4) support for SSD168x panels, starting with `GDEY0266T90`:
  `Gray4Registers`/`EpdPanel::GRAY4` (`src/traits.rs`), `Ssd168xController::with_gray4`/`gray4()`
  and `Ssd168xRefreshMode::Gray4` (`src/controllers/ssd168x.rs`), and `Gray4Color`/
  `Gray4Polarity`/`GrayBufferPair`/`render_paged_gray4` (`src/graphics`) — structural mirrors of
  the existing Tri-Color `TriColor`/`PlanePolarity`/`PageBufferPair`/`render_paged_tri_color`.

  **Provenance, worth stating plainly**: `GDEY0266T90::GRAY4` is *not* Good Display/Waveshare
  material — Waveshare's own spec lists 2 grayscale levels, and the GxEPD2 reference driver never
  writes a grayscale LUT. It is transcribed verbatim from Adafruit_EPD's
  `ThinkInk_266_Grayscale4_MFGN` reference driver (`ti_266mfgn_gray4_init_code`/
  `ti_266mfgn_gray4_lut_code`, a 233-byte custom waveform LUT), confirmed rendering four distinct
  gray levels on real hardware (a XIAO MG24 driving this exact panel). One register in the bundle
  (`0x3F`) is undocumented by Adafruit's own source (`// ???`) but is confirmed against the actual
  SSD1680 datasheet (Solomon Systech Rev 0.14) as "Option for LUT end" — a real, named register,
  part of the same unified waveform-setting block as the LUT/gate/source/VCOM registers, not a
  magic number. Gray4 is opt-in only: `for_panel` does not read `GRAY4` automatically, since it is
  a wholly different, mutually exclusive waveform configuration rather than an additive tweak like
  `VCOM`/`GATE_VOLTAGE`/`CUSTOM_LUT`. **Hardware-verified through `epdsi`'s own driver**, blocking
  on RP2350, RP2040 and ESP32-C3 and async on RP2350 (same four repos as above) — all four gray
  levels render distinctly on real glass on every host, not just through Adafruit's own driver.

## [0.2.2] - 2026-09-12

Tri-Color panels now have two supported drawing modes — `PageBufferPair` (below) is the
recommended default for new code; the existing manual dual-`PageBuffer` approach remains fully
supported for windowed/partial-region updates and refresh-mode timing work. Both modes are
hardware-verified side by side: every Tri-Color hardware example in this crate's downstream
example repos gained a `PageBufferPair`-based sibling reproducing the same content, and all 12
(across RP2350 blocking/async, RP2040, and ESP32-C3, covering all three Tri-Color panels `epdsi`
ships) were flashed and confirmed working before this entry was written.

### Added

- `TriColor`, `PlanePolarity` and `PageBufferPair` (`epdsi::graphics::buffer`), an
  `embedded-graphics` `DrawTarget<Color = TriColor>` that addresses a Tri-Color panel's
  Black/White and accent (red/yellow) RAM planes together, so a single drawing pass routes each
  pixel to the correct plane instead of requiring two separate `render_paged` calls with a
  `BinaryColor` closure apiece. `TriColor` has three variants — `White`, `Black`, `Accent` —
  mirroring the three-color palette the panel itself renders.

  `PlanePolarity` is a required constructor argument, not a hardcoded assumption: `epdsi`'s own
  Tri-Color panels disagree on which raw RAM bit means "ink." On SSD1680/SSD1681
  (`GDEY0266Z90`, `GDEM0154Z90`) the Black/White plane is normal but the accent plane is
  inverted (a *set* bit is red/yellow); on UC8253 (`SE0352N14TNGA0`) *both* planes are inverted.
  `PlanePolarity::SSD168X` and `PlanePolarity::UC8253` cover both, verified against the existing
  hardware examples' documented polarity for each panel.
- `PageBufferPair::clear`, resetting both planes to their own background fill under the pair's
  `polarity` — the two-plane equivalent of `PageBuffer::clear_byte`, with no fill byte for the
  caller to get wrong, needed for a manually-driven windowed/partial-region redraw (the pattern
  `ssd1680_gdey0266z90_epd`'s Phase 2/4 band updates use) to reset its buffer between draws.
- `render_paged_tri_color` (`epdsi::graphics::paged`), the `PageBufferPair` counterpart of
  `render_paged`: same page-by-page sweep, but writes both planes to their own channel
  (`ColorChannel::BlackWhite` and a caller-supplied accent channel, `ColorChannel::RedYellow`
  for every Tri-Color panel `epdsi` ships) before advancing to the next page. Each plane's
  background fill byte is derived from `PlanePolarity` rather than taken as a separate parameter,
  so it can't drift out of sync with the polarity used to draw. Purely additive — `render_paged`
  is untouched, and no controller changes were needed.

## [0.2.1] - 2026-09-05

### Fixed

- The optional `defmt` feature depended on `defmt = "0.3"`, while every downstream project
  exercising it (all five hardware-example repos) depends on `defmt = "1.0"` directly. Cargo
  happily built both major versions side by side, so this compiled — but the `defmt::Format`
  epdsi derived came from a different, incompatible `defmt` crate instance than the one a
  consumer's own `info!`/`error!`/`assert!` macros resolve `Format` from, making the feature
  silently unusable by any real consumer despite compiling cleanly. Bumped to `defmt = "1.0"`
  to match the ecosystem; no source changes were needed. Non-breaking: `defmt` is optional and
  gated behind the `defmt` feature, and the derive usage is unchanged.

## [0.2.0] - 2026-09-05

Phase 5 of the parity remediation plan — the one breaking release. Three changes:

Every panel across both APIs has been flashed and hardware-verified before this release: three
blocking hosts (RP2350 `rust-rpico2-discovery`, Feather RP2040 `adafruit-feather-thinkink-discovery`,
XIAO ESP32-C3 `xiao-esp32c3-blinky`) and two async hosts (RP2350 `rust-rpico2-embassy-examples`,
XIAO ESP32-S3 `rust-reterminal-e1002-examples`), covering all eleven panels this crate ships a
controller for, including the four Pervasive Displays panels on the EXT3-1 extension board. The
XIAO ESP32-C3 module used for the 0.1.4/0.1.5 bring-up was found to be faulty (see that entry
below); a replacement module now runs every non-Pervasive panel cleanly, matching RP2350/RP2040
timings — see the README's "A different microcontroller" section for the resolved writeup. See the
README's "Examples on real hardware" table for the full per-panel, per-host verification matrix.

### Added

- An async API via `embedded-hal-async`, alongside the existing blocking one. Controlled by the
  new `blocking` Cargo feature (**on by default**, so existing `Cargo.toml`s keep compiling
  unchanged); disabling it (`default-features = false`, then re-add `graphics` if wanted) switches
  every controller/bus/driver method to its async counterpart — same types, same method names,
  same `Result`s, just `.await`ed. Generated from one source per controller via native
  async-fn-in-trait (stable since this crate's MSRV, 1.75; verified against 1.75 directly, not
  just stable). See the crate root doc's "Cargo features" section for the two behavior
  differences worth knowing (async busy-waits have no timeout; `render_paged`'s drawing closure
  stays synchronous in both modes).

### Changed (breaking)

- `EpdBusError` gains two variants — `BufferTooSmall { required, provided }` and
  `InvalidWindow` — and is now `#[non_exhaustive]`. `EpdDriver::write_frame` rejects a buffer
  shorter than the active window/channel requires; `EpdDriver::set_window` rejects an inverted
  range or one outside the panel's declared dimensions. Both reject before anything reaches the
  bus. `EpdController::Error` now requires `From<ValidationError>` — satisfied automatically by
  every controller `epdsi` ships; only relevant to an out-of-tree `EpdController` implementation,
  and none is known to exist.
- Removed the three `EpdPanel` methods (`vcom`/`custom_lut`/`gate_voltage`) deprecated since
  0.1.6. Use the `VCOM`/`CUSTOM_LUT`/`GATE_VOLTAGE` associated consts instead — every panel
  `epdsi` ships already does.

## [0.1.7] - 2026-09-02

### Changed

- `Ssd1677Controller` clears a colour plane with the controller's own RAM pattern generator
  (`0x46` / `0x47`) instead of streaming every byte. A full-plane clear on an 800 × 480
  `GDEQ0426T82` now costs one command and one data byte rather than 48,000 streamed bytes.

  Measured on an RP2350 at a 16 MHz SPI clock: **4 658 µs against 34 469 µs**, a 7.4× saving of
  about 29.8 ms per plane clear. The remaining 4.7 ms is the controller's own internal sweep;
  what disappears is the 24 ms of SPI line time plus the overhead of 750 chunked writes.
  Verified on hardware against both paths on RP2350 and RP2040 — a cleared plane renders
  identically either way.

  These registers drive a *regular pattern*, not a memset — `A[7]` is one step's value, `A[6:4]`
  the step height in gates (max 680) and `A[2:0]` the step width in sources (max 960) — so the
  fast path is taken only where the result is provably uniform: a `0x00` or `0xFF` fill, covering
  a whole plane, on a panel inside 960 × 680. `0xF7` and `0x77` are consequently the only two
  bytes ever sent. Everything else streams exactly as before, including partial fills and
  non-uniform bytes. BUSY is waited on, since the sweep runs in hardware, and the RAM address
  counter is re-seated to the window origin afterwards — streaming a plane left it wrapped back
  there on its own, so without that a caller writing image data straight after a clear would
  render it displaced rather than faster.

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

[Unreleased]: https://github.com/melastmohican/epdsi/compare/v0.4.2...HEAD
[0.4.2]: https://github.com/melastmohican/epdsi/compare/v0.4.1...v0.4.2
[0.4.1]: https://github.com/melastmohican/epdsi/compare/v0.4.0...v0.4.1
[0.4.0]: https://github.com/melastmohican/epdsi/compare/v0.3.1...v0.4.0
[0.3.1]: https://github.com/melastmohican/epdsi/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/melastmohican/epdsi/compare/v0.2.2...v0.3.0
[0.2.2]: https://github.com/melastmohican/epdsi/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/melastmohican/epdsi/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/melastmohican/epdsi/compare/v0.1.7...v0.2.0
[0.1.7]: https://github.com/melastmohican/epdsi/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/melastmohican/epdsi/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/melastmohican/epdsi/compare/v0.1.4...v0.1.5
[0.1.4]: https://github.com/melastmohican/epdsi/compare/v0.1.3...v0.1.4
[0.1.3]: https://github.com/melastmohican/epdsi/compare/v0.1.2...v0.1.3
[0.1.2]: https://github.com/melastmohican/epdsi/compare/v0.1.1...v0.1.2
[0.1.1]: https://github.com/melastmohican/epdsi/compare/v0.1.0...v0.1.1
[0.1.0]: https://github.com/melastmohican/epdsi/releases/tag/v0.1.0

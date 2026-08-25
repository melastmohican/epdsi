# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## What this is

`epdsi` is a `no_std`, `embedded-hal` 1.0 compatible Rust driver framework for Electronic Paper Displays (EPD). It targets real embedded hardware (RP2040, STM32/Cortex-M4F, ESP32-C3/C6) as well as `std` test builds.

## Common commands

```bash
cargo check --all-targets                       # matches CI "check" job
cargo test --workspace                           # run all tests
cargo test --test pervasive_tests                # run a single test file
cargo test --test pervasive_tests <test_name>    # run a single test
cargo clippy --all-features --workspace -- -D warnings   # matches CI "clippy" job (warnings are hard errors)
cargo doc --no-deps --all-features               # matches CI "doc" job
cargo check --target thumbv6m-none-eabi          # RP2040 (Cortex-M0+) cross-compile check
cargo check --target thumbv7em-none-eabihf        # STM32/Cortex-M4F cross-compile check
cargo check --target riscv32imac-unknown-none-elf # ESP32-C3/C6 cross-compile check
```

CI (`.github/workflows/ci.yml`) runs `check`, `test`, `clippy` (deny warnings), and `doc` on `ubuntu-latest`, plus a separate cross-compile `check` job across the three targets above. There is no `std` feature flag — cross-target checks are plain `cargo check --target ...` against the `no_std` lib.

## Architecture

The crate cleanly separates three concerns via traits, so a new display is normally "one controller + one panel", not a new driver from scratch:

- **`EpdPanel`** (`src/traits.rs`) — a zero-sized type describing static physical panel facts: `WIDTH`, `HEIGHT`, `COLOR_MODE`, and optional overrides (`vcom()`, `custom_lut()`, `gate_voltage()`). Panels live in `src/panels/*.rs`, one file per panel, re-exported from `src/panels/mod.rs`. Many panels also expose a Pervasive-reference type alias (e.g. `EPD_266_KS_0C = E2266KS0C1`) for parity with vendor naming.
- **`EpdController<BUS>`** (`src/traits.rs`) — the driver-IC command/register logic: `init_sequence`, `set_window`, `set_cursor`, `write_frame`, `write_frame_pattern`, `trigger_refresh`, `sleep`. Controllers live in `src/controllers/*.rs`, one per driver IC (SSD1680, SSD1681, SSD1677, UC8253, ED2208, JD79660, JD79661, Pervasive Displays COG), re-exported from `src/controllers/mod.rs`.
- **`SpiBusWrapper`** (`src/bus.rs`) — the physical transport: wraps an `embedded-hal` `SpiDevice` plus DC/RST/BUSY GPIO pins. Provides `send_command`, `send_data`, `send_command_with_data`, `send_data_repeated`, `hard_reset`, and both spin-loop (`wait_busy`) and delay-based (`wait_busy_with_delay`, capped at 1500 iterations) busy-polling. `busy_active_high` must match the panel's actual busy polarity (some panels are active-low).
- **`EpdDriver<BUS, CONTROLLER, PANEL>`** (`src/driver.rs`) — the orchestrator that ties a bus, a controller instance, and a panel type (via `PhantomData`) together, and exposes the public API (`init`, `set_window`, `write_frame`, `clear_frame`, `refresh`, `sleep`). Built via `EpdBuilder::<CONTROLLER, PANEL>::new(controller).build(bus)`.
- **`ColorChannel`** / **`ColorMode`** / **`SevenColor`** (`src/traits.rs`) — a unified color model across Monochrome, Tri-Color (B/W/Red), Quad-Color (2bpp), and 7-Color ACeP (4bpp, packed two pixels per byte via `SevenColor::pack`). `write_frame`/`clear_frame` always take a `ColorChannel` so multi-buffer COGs (e.g. Pervasive's B/W + Red RAM) are addressed explicitly.
- **Paged rendering** (`src/graphics/`) — `PageBuffer` (`buffer.rs`) is a small stack-sized `DrawTarget` (behind the `graphics` feature, using `embedded-graphics-core`) representing one horizontal page of the frame. `render_paged` (`paged.rs`) implements the GxEPD2-style pattern: sweep the panel page-by-page, let a user closure draw into each `PageBuffer`, then `set_window`/`set_cursor`/`write_frame` that page and `refresh` once at the end — this keeps RAM usage tiny (`div_ceil` protects against non-page-aligned heights).
- **`prelude`** (`src/prelude.rs`) — the intended single import for consumers; re-exports the traits, controllers, panels, and driver/builder types.

Adding a new panel to an existing controller IC only requires a new file in `src/panels/` implementing `EpdPanel`. Adding a new driver IC requires a new file in `src/controllers/` implementing `EpdController<BUS>` for the generic `SpiBusWrapper<SPI, DC, RST, BUSY>`.

### Testing patterns

- `tests/ssd1680_tests.rs`, `tests/ssd1681_tests.rs`, `tests/ssd1677_tests.rs`, `tests/uc8253_tests.rs`, `tests/ed2208_tests.rs`, `tests/pervasive_tests.rs`, `tests/pervasive_bwry_tests.rs`, `tests/compile_tests.rs` use a hand-rolled `RecordingSpiBus` (wrapping `RefCell<Vec<SpiRecord>>`, `SpiRecord::{Command, Data}`) that records DC-pin state to distinguish command bytes from data bytes, so tests can assert exact SPI byte sequences sent to the panel. `embedded-hal-mock` is also available as a dev-dependency for pin/delay mocking.
- When changing a controller's SPI sequence, prefer asserting the exact recorded byte stream rather than just "it doesn't error" — register-level parity with vendor reference drivers is the point of this crate.

### Parity auditing skills

- **Pervasive Displays parity auditing**: The `pervasive-parity-audit` skill (`.agents/skills/pervasive-parity-audit/SKILL.md`) documents the checkpoints for keeping `src/controllers/pervasive_bw.rs` (`PervasiveBwController`, DriverC/DriverF) and `src/controllers/pervasive_bwry.rs` (`PervasiveBwryController`, DriverF/DriverA, OTP registers read via a bit-banged 3-wire handshake — see `crate::bus3::Spi3Bus`) in register-level parity with official Pervasive Displays C++ reference drivers (`Pervasive_Wide_Small`, `Pervasive_BWRY_Small`, `PDLS_Common`).
- **GxEPD2 parity auditing**: The `gxepd2-parity-audit` skill (`.agents/skills/gxepd2-parity-audit/SKILL.md`) documents the checkpoints for auditing non-Pervasive controllers (`SSD1680`, `SSD1681`, `SSD1677`, `UC8253`, `ED2208`, `JD79660`, `JD79661`) against `ZinggJM/GxEPD2` C++ reference drivers. Consult it before modifying controller command sequences, RAM window bounds, custom waveform LUT tables, or update modes (`Full`, `Partial`, `FastFull`, `FastPartial`).

### Hardware note

EXT3-1 extension boards: the **J3 jumper** must be **OPEN** (10 µH path) for panels ≤ 3.7" (e.g. 2.66", 2.9"). Closed (47 µH path) causes DC-DC booster power sag and BUSY-pin hangs on small panels — relevant when debugging reported hardware behavior, not something code can fix.


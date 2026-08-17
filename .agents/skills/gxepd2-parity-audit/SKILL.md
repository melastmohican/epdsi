---
name: gxepd2-parity-audit
description: Verifies panel identity against Good Display / Waveshare product pages and datasheets, then audits feature, register, lookup table (LUT), and command-sequence parity between the GxEPD2 C++ reference driver library (ZinggJM/GxEPD2) and the Rust epdsi library implementations (covering SSD1680, SSD1681, SSD1677, UC8253, ED2208, and JD79661 controllers).
---

# GxEPD2 C++ / Rust Parity Audit

This skill provides step-by-step instructions for auditing, validating, and maintaining parity between the official [GxEPD2](https://github.com/ZinggJM/GxEPD2) C++ reference driver library and the Rust `epdsi` library implementation. `epdsi` implements several non-Pervasive display controllers based on GxEPD2 reference drivers:

| `epdsi` controller | File | Panel / GxEPD2 alias | GxEPD2 C++ reference header/cpp |
| :--- | :--- | :--- | :--- |
| `Ssd1680Controller` / `Ssd168xController` | `src/controllers/ssd168x.rs` | `GDEM0213B74` (`GxEPD2_213_B74`) | `src/epd/GxEPD2_213_B74.h` & `.cpp` |
| `Ssd1681Controller` / `Ssd168xController` | `src/controllers/ssd168x.rs` | `GDEM0154Z90` (`GxEPD2_154_D67`) | `src/epd3c/GxEPD2_154_D67.h` & `.cpp` |
| `Ssd1677Controller` | `src/controllers/ssd1677.rs` | `GDEQ0426T82` (`GxEPD2_426_GDEQ0426T82`) | `src/epd/GxEPD2_426_GDEQ0426T82.h` & `.cpp` |
| `Uc8253Controller` | `src/controllers/uc8253.rs` | `GDEY037T03` (`GxEPD2_370_GDEY037T03`) | `src/epd/GxEPD2_370_GDEY037T03.h` & `.cpp` |
| `Ed2208Controller` | `src/controllers/ed2208.rs` | `GDEP073E01` (`GxEPD2_730c_GDEP073E01`) | `src/epd7c/GxEPD2_730c_GDEP073E01.h` & `.cpp` |
| `Jd79661Controller` | `src/controllers/jd79661.rs` | `ZJY122250` / `GDEY0213F51` (`GxEPD2_213c_GDEY0213F51`) | `src/epd4c/GxEPD2_213c_GDEY0213F51.h` & `.cpp` |

---

## Reference Repositories

When conducting a GxEPD2 parity audit, ensure the `GxEPD2` reference repository is checked out locally in a temporary directory:
- [ZinggJM/GxEPD2](https://github.com/ZinggJM/GxEPD2) (`~/Src/tmp/GxEPD2`)

Key reference directories and files in GxEPD2 to inspect:
- `src/GxEPD2_EPD.h` & `src/GxEPD2_EPD.cpp` (Base display driver class, CS/DC/RST handling, SPI framing)
- `src/epd/` (Monochrome BW display drivers, e.g. `GxEPD2_213_B74.cpp`, `GxEPD2_370_GDEY037T03.cpp`, `GxEPD2_426_GDEQ0426T82.cpp`)
- `src/epd3c/` (3-Color BWR/BWY display drivers, e.g. `GxEPD2_154_D67.cpp`)
- `src/epd4c/` (4-Color BWRY display drivers, e.g. `GxEPD2_213c_GDEY0213F51.cpp`)
- `src/epd7c/` (7-Color ACeP / Spectra 6 display drivers, e.g. `GxEPD2_730c_GDEP073E01.cpp`)

---

## Panel Identification Verification (mandatory, BEFORE any register-level work)

Before auditing or writing code for a Good Display / Waveshare panel, verify panel identity against vendor datasheets and GxEPD2 definitions:

1. **Fetch the vendor product page / datasheet** (Good Display `https://www.good-display.com` or Waveshare wiki). Record the exact part number, native resolution, color depth, controller IC model, and active area dimensions.
2. **Locate the exact GxEPD2 driver class** in `src/epd/`, `src/epd3c/`, `src/epd4c/`, or `src/epd7c/` matching the panel model name and resolution.
3. **Verify resolution and RAM window bounds**: Check whether physical panel resolution differs from controller IC internal RAM dimensions (e.g. 122×250 panel on a 128×296 RAM controller like SSD1680). Confirm X/Y start/end offsets and gate/source drive counts.
4. **Confirm GxEPD2 Type Alias**: Ensure `epdsi` exports an explicit GxEPD2-compatible type alias (e.g., `pub type GxEPD2_213_B74 = GDEM0213B74;`).

---

## Checkpoints Matrix — Controller Families

### 1. SSD1680 Variant (`src/controllers/ssd168x.rs`)
- **Reference**: `src/epd/GxEPD2_213_B74.h` & `.cpp` (`GxEPD2_213_B74`)
- **Driver Output Control (`0x01`)**: Gate count = `(HEIGHT - 1) & 0xFF`, `(HEIGHT - 1) >> 8`, scan sequence `0x00` (or `0x01`/`0x03` depending on pin orientation).
- **Data Entry Mode (`0x11`)**: `0x03` (X increment, Y increment).
- **RAM Window & Address Counters (`0x44`, `0x45`, `0x4E`, `0x4F`)**:
  - `0x44` (RAM X start/end): `x_start / 8` to `x_end / 8`.
  - `0x45` (RAM Y start/end): `y_start & 0xFF`, `y_start >> 8` to `y_end & 0xFF`, `y_end >> 8`.
  - `0x4E` / `0x4F` (RAM X/Y counter): set to start offsets prior to data write.
- **Display Update Control (`0x22` + `0x20`)**:
  - Full refresh sequence: `0xF7` or `0xC7` + `0x20` Master Activation.
  - Partial refresh sequence: `0xFF` or custom LUT write + `0x0C` / `0xC7` + `0x20` Master Activation.
- **Deep Sleep (`0x10`)**: payload `&[0x01]` (Mode 1 deep sleep).

### 2. SSD1677 Controller (`src/controllers/ssd1677.rs`)
- **Reference**: `src/epd/GxEPD2_426_GDEQ0426T82.h` & `.cpp` (`GxEPD2_426_GDEQ0426T82`)
- **Panel Setting (`0x00`)**: Resets and configures bus/display format.
- **Power Setting (`0x01`)**: VGH/VGL boost stages and driver supply voltages.
- **Booster Soft Start (`0x06`)**: Soft-start phase parameters for large 4.26" panels.
- **Resolution Setting (`0x61`)**: `width` high/low byte, `height` high/low byte.
- **VCOM & Data Interval (`0x50`)**: Border waveform control (`0x17`/`0x37`).
- **Display Update Control 2 (`0x22` + `0x20`)**: Master activation trigger.

### 3. UC8253 Controller (`src/controllers/uc8253.rs`)
- **Reference**: `src/epd/GxEPD2_370_GDEY037T03.h` & `.cpp` (`GxEPD2_370_GDEY037T03`)
- **Panel Setting (`0x00`)**: `0x0E` soft reset; operational configuration `0x1F` / `0x0F` / `0x5F` (depending on refresh mode).
- **Refresh Modes**:
  - `Normal` (Full update): Uses OTP LUTs.
  - `Fast` (Fast full update): Custom fast update LUT table written to register `0x32`.
  - `Partial` / `FastPartial`: Fast partial refresh sequence with custom LUTs for 3.7" panel.
- **Power Control (`0x01`, `0x02`, `0x04`)**: Power off `0x02`, Power on `0x04`.

### 4. ED2208 Controller (`src/controllers/ed2208.rs`)
- **Reference**: `src/epd7c/GxEPD2_730c_GDEP073E01.h` & `.cpp` (`GxEPD2_730c_GDEP073E01`)
- **Panel / Tech**: 7.3" 7-Color ACeP / Spectra 6 (`GDEP073E01`, 800×480).
- **Command Sequence**:
  - `PSR` (`0x00`): Panel Setting Register.
  - `PWR` (`0x01`): Power setting sequence.
  - `PFC` (`0x03`): Power off sequence.
  - `OVD/DRV` (`0x06`): Driver voltage settings.
  - `TCON` (`0x61`): Resolution setting (800 × 480).
  - `CDI` (`0x50`): VCOM and Data Interval setting.
  - `PON` (`0x04`): Power on trigger + wait busy active-low.
  - `DRF` (`0x12`): Display refresh + wait busy active-low.
- **Pixel Packing**: 4 bits-per-pixel (or 3 bits-per-pixel packed), 2 pixels per byte, encoding 7 discrete color codes (Black=0, White=1, Green=2, Blue=3, Red=4, Yellow=5, Orange=6, Clean=7).

### 5. JD79661 Controller (`src/controllers/jd79661.rs`)
- **Reference**: `src/epd4c/GxEPD2_213c_GDEY0213F51.h` & `.cpp` (`GxEPD2_213c_GDEY0213F51`)
- **Panel / Tech**: 4-Color (Black, White, Red, Yellow) 2.13" e-Paper (`ZJY122250` / `GDEY0213F51`).
- **Command Sequence**:
  - `PSR` (`0x00`), `PWR` (`0x01`), `POF` (`0x02`), `PON` (`0x04`), `SRES` (`0x61`), `CDI` (`0x50`).
- **Pixel Streaming**: 2 bits-per-pixel packed 4-color format streamed via command `0x10`.

### 6. SSD1681 Variant (`src/controllers/ssd168x.rs`)
- **Reference**: `src/epd3c/GxEPD2_154_D67.h` & `.cpp` (`GxEPD2_154_D67`)
- **Panel / Tech**: 3-Color BWR 1.54" e-Paper (`GDEM0154Z90`).
- **Driver Output Control (`0x01`)**: Gate height configuration `(HEIGHT - 1) & 0xFF`, `(HEIGHT - 1) >> 8`, scan sequence byte (`0x00`).
- **Data Entry Mode (`0x11`)**: `0x03` (X increment, Y increment).
- **RAM Position & Counters (`0x44`, `0x45`, `0x4E`, `0x4F`)**: RAM X start/end position, RAM Y start/end position, and address counter initialization.
- **Display Update Control 2 (`0x22` + `0x20`)**: Payload `&[0xF7]` or `&[0xFC]` + `cmd::MASTER_ACTIVATE` (`0x20`).
- **Deep Sleep (`0x10`)**: Payload `&[0x01]` (Mode 1 deep sleep).


### 7. Refresh Mode Capability Audit (Full, Partial, Fast, FastPartial)
- **Rule**: For every audited controller, check which update modes are supported by the hardware and reference C++ library:
  - **Full Refresh** (Standard OTP LUT refresh).
  - **Partial Refresh** (Sub-window RAM update with partial waveform LUT, e.g. `UPDATE_DISPLAY_CTRL2 = 0xFC` or custom partial LUT).
  - **Fast Full / Fast Partial Refresh** (Fast update mode via custom LUT written to register `0x32`, temperature register override `0x1A`, or differential fast update).
- **Verification**:
  - Confirm `epdsi` exposes an explicit builder enum/setting for multi-mode controllers (e.g. `Ssd1680RefreshMode::{Full, Partial}`, `Ssd1677RefreshMode::{Full, FastFull, Partial}`, `Uc8253RefreshMode::{Normal, Fast, Partial, FastPartial}`, `PervasiveRefreshMode::{Normal, Fast}`).
  - Confirm custom waveform tables (e.g. UC8253 `LUT_FAST` array in `uc8253.rs`) match C++ reference byte-for-byte.
  - Flag any refresh modes supported by the C++ reference driver but missing in `epdsi` as explicit parity gaps.

---

## Verification Workflow

1. **Verify panel identity** against Good Display / Waveshare product page and datasheet.
2. **Locate GxEPD2 reference source** in `~/Src/tmp/GxEPD2/src/epd*/`.
3. **Audit register sequence & payloads**: Compare every command byte and data array in `epdsi` controller methods (`init_sequence`, `write_frame`, `trigger_refresh`, `sleep`) against GxEPD2's `_InitDisplay()`, `_Update_Full()`, `_Update_Part()`, and `_PowerOff()`.
4. **Audit Refresh Modes**: Verify all supported refresh modes (Full, Partial, Fast) are implemented and custom LUT arrays match C++ references byte-for-byte.
5. **Run Automated Tests**:
   ```bash
   cargo test --test ssd1680_tests
   cargo test --test ssd1681_tests
   cargo test --test ssd1677_tests
   cargo test --test uc8253_tests
   cargo test --test ed2208_tests
   cargo test --test compile_tests
   ```

6. **Verify GxEPD2 Type Aliases**: Ensure tests verify that exported GxEPD2 type aliases resolve to the expected panel struct and match `WIDTH`/`HEIGHT` constants.



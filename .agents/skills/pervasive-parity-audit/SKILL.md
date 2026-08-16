---
name: pervasive-parity-audit
description: Audits and verifies feature and register-level parity between Pervasive Displays C++ reference drivers (Pervasive_Wide_Small, Pervasive_BWRY_Small, PDLS_Common) and the Rust epdsi library implementation, covering both the BW/BWR (PervasiveBwController) and BWRY (PervasiveBwryController) driver families.
---

# Pervasive Displays Driver C++ / Rust Parity Audit

This skill provides step-by-step instructions for auditing, validating, and maintaining parity between official Pervasive Displays C++ reference drivers and the Rust `epdsi` library implementation. `epdsi` implements **two** distinct Pervasive controllers — pick the matching reference repo and checkpoints section for whichever one you're auditing:

| epdsi controller | File | COG family | Reference repo |
| :--- | :--- | :--- | :--- |
| `PervasiveBwController` | `src/controllers/pervasive_bw.rs` | BW/BWR (DriverC/DriverF) | `Pervasive_Wide_Small` |
| `PervasiveBwryController` | `src/controllers/pervasive_bwry.rs` | BWRY/Spectra-4 (Driver6/DriverA) | `Pervasive_BWRY_Small` |

These are genuinely different register conventions (different temperature command, different frame-plane layout, hardcoded vs. OTP-sourced registers — see §9) — do not cross-apply a rule from one family's checkpoints to the other without checking its own reference source first.

## Reference Repositories

When conducting a parity audit, ensure reference C++ implementations are checked out locally in a temporary directory (e.g., `~/Src/tmp`):
- [Pervasive_Wide_Small](https://github.com/PervasiveDisplays/Pervasive_Wide_Small) — BW/BWR family (`PervasiveBwController`)
- [Pervasive_BWRY_Small](https://github.com/PervasiveDisplays/Pervasive_BWRY_Small) — BWRY family (`PervasiveBwryController`)
- [PDLS_Common](https://github.com/rei-vilo/PDLS_Common) — shared HAL/board layer used by both

Key reference C++ source files to inspect:
- `Pervasive_Wide_Small.cpp` & `Pervasive_Wide_Small.h` (BW/BWR)
- `Pervasive_BWRY_Small.cpp` & `Pervasive_BWRY_Small.h` (BWRY)
- `Driver_EPD_Virtual.cpp` & `Driver_EPD_Virtual.h`
- `hV_Board.cpp` & `hV_Board.h`
- `hV_HAL_Peripherals.cpp`
- `hV_List_Constants.h` (screen/driver-variant macros, e.g. `eScreen_EPD_152_QS_06`, `eScreen_EPD_417_QS_0A`)

---

## Checkpoints Matrix — BW/BWR Family (`PervasiveBwController`)

### 1. SPI Command Byte Payloads
- **Rule**: Commands `POWER_ON` (`0x04`), `DISPLAY_REFRESH` (`0x12`), and `POWER_OFF` (`0x02`) must **NOT** include trailing payload bytes.
- **Verification**: Ensure Rust code calls `bus.send_command(cmd)` rather than `send_command_with_data(cmd, &[0x00])`.

### 2. Differential Fast Update Frame Routing
- **Rule**:
  - `0x10` (`WRITE_BW_DATA` / DTM1): Receives **Previous / Old** image frame (`!b` bitwise inverted for display logic).
  - `0x13` (`WRITE_RED_DATA` / DTM2): Receives **Next / Current** image frame (`!b` bitwise inverted for display logic).
- **C++ Reference**: `b_sendIndexData(0x10, secondFrame, sizeFrame)` (secondFrame = old image), `b_sendIndexData(0x13, firstFrame, sizeFrame)` (firstFrame = new image).

### 3. VCOM & Data Interval Setting (`0x50`) Toggling
- **Rule**: During `write_fast_frame`:
  - Send `cmd::VCOM_INTERVAL` (`0x50`) with payload `&[0x27]` before streaming frame data.
  - Send `cmd::VCOM_INTERVAL` (`0x50`) with payload `&[0x07]` after streaming frame data.

### 4. Ambient Temperature Compensation
- **Rule**:
  - `INPUT_TEMP` (`0xE5`):
    - Normal update mode: `temperature_c as u8`.
    - Fast update mode: `(temperature_c as u8) | 0x40`.
  - `ACTIVE_TEMP` (`0xE0`): payload `&[0x02]`.

### 5. Driver IC Variant & Panel Setting Register (PSR `0x00`) Configuration
- **Rule**:
  - Soft reset: `cmd::PSR` (`0x00`) with payload `&[0x0E]`.
  - `PervasiveDriverVariant::DriverC` (e.g., `E2266KS0C1` / `EPD_266_KS_0C`):
    - Normal update PSR: `[psr[0], psr[1]]`.
    - Fast update PSR: `[psr[0] | 0x10, psr[1] | 0x02]`.
  - `PervasiveDriverVariant::DriverF` (e.g., `E2290KS0F1` / `EPD_290_KS_0F`):
    - Skips PSR `0x00` calibration bytes.
    - Sends command `0x4D` with payload `&[0x55]`.
    - Sends command `0xE9` with payload `&[0x02]`.

### 6. Secondary RAM Buffer Clearance
- **Rule**: In normal update mode, writing `ColorChannel::BlackWhite` data to `0x10` must automatically clear `WRITE_RED_DATA` (`0x13`) with `0x00` bytes to prevent RAM noise on multi-buffer COGs.

### 7. Pervasive Reference Type Aliases
- **Rule**: Provide type aliases matching Pervasive C++ screen definitions:
  - `pub type EPD_266_KS_0C = E2266KS0C1;`
  - `pub type EPD_290_KS_0F = E2290KS0F1;`

---

## Checkpoints Matrix — BWRY Family (`PervasiveBwryController`)

`PervasiveBwryController` (`src/controllers/pervasive_bwry.rs`) is a **separate** controller from `PervasiveBwController` above — it drives the Spectra-4/BWRY (Black/White/Red/Yellow) COG family (`Pervasive_BWRY_Small` reference, not `Pervasive_Wide_Small`/`PDLS_Common`'s BW/BWR family), and its register conventions differ enough from DriverC/DriverF that they must not be cross-applied:

### 9. Temperature Register
- **Rule**: BWRY uses `INPUT_TEMP = 0xE6`, **not** the BW/BWR controller's `cmd::INPUT_TEMP = 0xE5`. This is the single most likely place a future edit could accidentally regress by copy-pasting from `pervasive_bw.rs`.

### 10. OTP-Sourced Registers (not hardcoded)
- **Rule**: Unlike DriverC's static `psr: [0xCF, 0x8D]`, BWRY's PSR, booster, PLL, CDI/VCOM, and resolution bytes are all sourced from a runtime OTP read (chip-ID handshake over `0x70`, then an unlock+read sequence into an `otp_data` buffer). Register offsets into `otp_data` differ between Driver6 (152, 48-byte OTP) and DriverA (417, 112-byte OTP, with a bank-2 fallback if the `0xA5` bank-start marker isn't found at offset 0).

### 11. Single-Plane Frame Format
- **Rule**: Frame data is one packed 2-bits-per-pixel buffer written entirely via `WRITE_DATA = 0x10` — there is no split `WRITE_BW_DATA`/`WRITE_RED_DATA` plane pair like the BW/BWR controller's `0x10`/`0x13`.

### 12. DriverA Power-On Timing
- **Rule**: DriverA sends `POWER_ON (0x04)` as the last step of its OTP-derived main init block (`init_sequence`); Driver6 defers `POWER_ON` to `trigger_refresh`. A parity check must confirm `POWER_ON` appears exactly once per full init→refresh cycle, at the correct stage for the variant in use.

### 13. Chip-ID Normalization
- **Rule**: A raw OTP chip-ID response of `0x8302` must be normalized to `0x0302` before comparing against the variant's expected ID (`0x4801` for Driver6, `0x0605` for DriverA) — a quirk carried over directly from the C++ reference's chip-ID check.

### 14. Extension Board J3 Jumper Configuration (EXT3-1)
- **Rule** (applies to both families — hardware-board-level, not COG-specific):
  - Small displays ($\le 3.7"$, e.g., 2.9" `E2290KS0F1`, 1.54" `E2154QS0F1`): **J3 Jumper OPEN** ($10\,\mu\text{H}$ inductor path).
  - Large displays ($> 3.7"$, e.g., 4.2" `E2417QS0A3`, 9.7"): **J3 Jumper CLOSED** ($47\,\mu\text{H}$ inductor path).
- **Troubleshooting**: If J3 is closed when driving a small display, DC-DC boost converter power sags during refresh, causing initialization failure or BUSY pin hangs.

---

## Verification Workflow

1. **Check C++ Reference Code**: Compare registers, timings, and byte streams against `Pervasive_Wide_Small.cpp` (BW/BWR) or `Pervasive_BWRY_Small.cpp` (BWRY), whichever family you're auditing.
2. **Run Automated Tests**:
   ```bash
   cargo test --test pervasive_tests       # BW/BWR (PervasiveBwController)
   cargo test --test pervasive_bwry_tests  # BWRY (PervasiveBwryController)
   ```
3. **Verify SPI Traces**: Ensure the mock SPI unit tests pass and confirm exact byte sequence order.
   - `tests/pervasive_tests.rs` uses the standard `RecordingSpiBus` (command/data only).
   - `tests/pervasive_bwry_tests.rs` uses its own `RecordingSpiBus` variant that additionally serves canned `SpiDevice::read` responses — required for the OTP handshake (bank-2 fallback, `InvalidOtpMarker` failure path); the standard `RecordingSpiBus` doesn't override `read()` and would silently no-op it.

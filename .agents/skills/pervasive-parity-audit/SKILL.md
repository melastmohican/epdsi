---
name: pervasive-parity-audit
description: Audits and verifies feature and register-level parity between Pervasive Displays C++ reference drivers (Pervasive_Wide_Small, PDLS_Common) and the Rust epdsi library implementation.
---

# Pervasive Displays Driver C++ / Rust Parity Audit

This skill provides step-by-step instructions for auditing, validating, and maintaining parity between official Pervasive Displays C++ reference drivers and the Rust `epdsi` library implementation.

## Reference Repositories

When conducting a parity audit, ensure reference C++ implementations are checked out locally in a temporary directory (e.g., `~/Src/tmp`):
- [Pervasive_Wide_Small](https://github.com/PervasiveDisplays/Pervasive_Wide_Small)
- [PDLS_Common](https://github.com/rei-vilo/PDLS_Common)

Key reference C++ source files to inspect:
- `Pervasive_Wide_Small.cpp` & `Pervasive_Wide_Small.h`
- `Driver_EPD_Virtual.cpp` & `Driver_EPD_Virtual.h`
- `hV_Board.cpp` & `hV_Board.h`
- `hV_HAL_Peripherals.cpp`

---

## Checkpoints Matrix

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

### 5. Panel Setting Register (PSR `0x00`) Configuration
- **Rule**:
  - Soft reset: `cmd::PSR` (`0x00`) with payload `&[0x0E]`.
  - Normal update PSR: `[psr[0], psr[1]]`.
  - Fast update PSR: `[psr[0] | 0x10, psr[1] | 0x02]`.

### 6. Secondary RAM Buffer Clearance
- **Rule**: In normal update mode, writing `ColorChannel::BlackWhite` data to `0x10` must automatically clear `WRITE_RED_DATA` (`0x13`) with `0x00` bytes to prevent RAM noise on multi-buffer COGs.

---

## Verification Workflow

1. **Check C++ Reference Code**: Compare registers, timings, and byte streams against `Pervasive_Wide_Small.cpp`.
2. **Run Automated Tests**:
   ```bash
   cargo test --test pervasive_tests
   ```
3. **Verify SPI Traces**: Ensure mock SPI unit tests in `tests/pervasive_tests.rs` pass and confirm exact byte sequence order.

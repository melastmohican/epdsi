---
name: pervasive-parity-audit
description: Verifies panel identity against Pervasive Displays' own product pages/datasheets, then audits feature and register-level parity between the C++ reference drivers (Pervasive_Wide_Small, Pervasive_BWRY_Small, PDLS_Common) and the Rust epdsi library implementation, covering both the BW/BWR (PervasiveBwController) and BWRY (PervasiveBwryController) driver families.
---

# Pervasive Displays Driver C++ / Rust Parity Audit

This skill provides step-by-step instructions for auditing, validating, and maintaining parity between official Pervasive Displays C++ reference drivers and the Rust `epdsi` library implementation. `epdsi` implements **two** distinct Pervasive controllers — pick the matching reference repo and checkpoints section for whichever one you're auditing:

| epdsi controller | File | COG family | Reference repo |
| :--- | :--- | :--- | :--- |
| `PervasiveBwController` | `src/controllers/pervasive_bw.rs` | BW/BWR (DriverC/DriverF) | `Pervasive_Wide_Small` |
| `PervasiveBwryController` | `src/controllers/pervasive_bwry.rs` | BWRY/Spectra-4 (DriverF/DriverA) | `Pervasive_BWRY_Small` |

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

## Panel Identification Verification (mandatory, BEFORE any register-level work)

The largest parity bug found in this codebase so far wasn't a byte-level transcription error — it was implementing the *correct* register sequence for the *wrong* panel. `E2154QS0F1` was originally implemented against `eScreen_EPD_152_QS_06` (Driver 6, chip ID `0x4801`, 200×200 overscanned RAM) per the initial task description, when the panel actually needed `eScreen_EPD_154_QS_0F` (Driver F, chip ID `0x0302`, 152×152 exact, no overscan). The Rust code was a byte-perfect match for the C++ `152_QS_06` case block, and the mock unit tests passed — because the implementation and the test expectations shared the same wrong assumption. **Only the physical panel's OTP chip-ID readback on real hardware caught it.** The register-level checkpoints below cannot catch this class of bug: they verify the Rust code matches *a given* C++ reference block, not whether that block was the right one to pick.

**Before writing a single register for a new panel or variant**, verify its identity against the vendor's own public sources — never trust a task description, a "looks similar" macro name, or numeric proximity (e.g. `152` vs `154`) without independently checking:

1. **Fetch the vendor's product page** for the panel's size category, e.g. `https://www.pervasivedisplays.com/products/{size}-e-ink-displays/` (append `#spectra-4` for BWRY parts, check the equivalent BWR/Spectra section otherwise). Find the exact part number and record its **"Screen/Driver Code"** verbatim (e.g. `154-QS-0F1` for `E2154QS0F1`) — this is Pervasive's own designation and must match the `eScreen_EPD_*` macro digit-for-digit (size, film, driver letter/number). A mismatch here (macro says `152`, vendor page says `154`) is the exact bug that shipped.
2. **Fetch the vendor's flyer/datasheet PDF** (linked from the product page) for the panel's stated **Resolution** (e.g. "152(H) x 152(V) pixel") — cross-check against whatever `WIDTH`/`HEIGHT` you're about to hardcode in the Rust panel file, and against the `frameSize_EPD_*` byte constant in `hV_List_Constants.h` (reverse the math: `bytes * 8 / bits_per_pixel` should equal `width * height`; don't assume overscan without a byte-size discrepancy to justify it).
3. **Locate the exact `case eScreen_EPD_<size>_<film>_<driver>:` block** in the C++ reference matching step 1's code — not a numerically- or visually-similar one — before transcribing any register sequence, `_chipId`, or `_readBytes` value from it.
4. Only once identity is confirmed by an external, vendor-authored source (not just the task description or prior research notes) should register-level transcription and the checkpoints below begin.

**Confirmed vendor links** for panels `epdsi` already ships (fetched and cross-checked against the Rust `WIDTH`/`HEIGHT` — re-verify here first if auditing one of these, and add a row here immediately after confirming a new panel):

| Panel | Product Page | Datasheet Flyer (PDF) | Confirmed Resolution |
| :--- | :--- | :--- | :--- |
| `E2154QS0F1` (BWRY, `154-QS-0F1`) | [link](https://www.pervasivedisplays.com/products/1-54-e-ink-displays/#spectra-4) | [link](https://www.pervasivedisplays.com/wp-content/uploads/2025/10/Flyer_E2154QS0F1_20241022.pdf) | 152(H) × 152(V) |
| `E2417QS0A3` (BWRY, `417-QS-0A`) | [link](https://www.pervasivedisplays.com/products/4-2-e-ink-displays/#spectra-4) | [link](https://www.pervasivedisplays.com/wp-content/uploads/2025/10/Flyer_E2417QS0A3_20250407.pdf) | 400(V) × 300(H) |
| `E2266KS0C1` (BW, `266-KS-0C`) | [link](https://www.pervasivedisplays.com/products/2-66-e-ink-displays/) | [link](https://www.pervasivedisplays.com/wp-content/uploads/2025/10/Flyer_E2266KS0C1_20241209.pdf) | 296(H) × 152(V) |
| `E2290KS0F1` (BW, `290-KS-0F`) | [link](https://www.pervasivedisplays.com/products/2-9-e-ink-displays/) | [link](https://www.pervasivedisplays.com/wp-content/uploads/2025/10/Flyer_E2290KS0F1_20241210.pdf) | 384(H) × 168(V) |

The same links also live in each panel's own doc comment (`src/panels/<name>.rs`, "Vendor References" section) — keep both copies in sync if a link changes.

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

### 9. OTP Read Is Bit-Banged 3-Wire, Not SPI
- **Rule**: The chip-ID/OTP register read (`Pervasive_BWRY_Small::COG_getDataOTP` in the C++ reference) does **not** use the hardware SPI peripheral. It bit-bangs `hV_HAL_SPI3_write`/`_read` over SCK + a single bidirectional DATA line (defaulting to the same physical wire as MOSI), switching that pin's direction between output and input per byte — the panel drives its OTP response back on MOSI, and MISO is never touched by this handshake. In `epdsi` this is modeled by `crate::bus3::Spi3Bus` + the `DynamicPin` trait, and exposed as `PervasiveBwryController::read_otp(&mut Spi3Bus, &mut delay)`, which callers must invoke once — using raw GPIO pins, before the hardware SPI peripheral is configured — prior to `EpdDriver::init()`.
- **Troubleshooting**: A chip-ID read that comes back as `0` (or any implausible value) on real hardware almost always means the OTP handshake is being attempted over the normal `SpiBusWrapper`/`SpiDevice` instead of `Spi3Bus` — this was exactly the root cause the first time this controller was tried on real hardware.
- **CS framing**: unlike the 4-wire write helpers (which may span multiple bytes under one call), the 3-wire bit-banged protocol pulses CS individually around **every single byte** (`Spi3Bus::write_cmd`/`write_data`/`read_data_byte`/`read_byte_no_dc` each does their own CS select/unselect) — do not batch multiple bytes under one CS assertion when porting new register sequences from the C++.

### 10. Temperature Register
- **Rule**: BWRY uses `INPUT_TEMP = 0xE6`, **not** the BW/BWR controller's `cmd::INPUT_TEMP = 0xE5`. This is the single most likely place a future edit could accidentally regress by copy-pasting from `pervasive_bw.rs`.

### 11. OTP-Sourced Registers (not hardcoded)
- **Rule**: Unlike DriverC's static `psr: [0xCF, 0x8D]`, BWRY's PSR, booster, PLL, CDI/VCOM, and resolution bytes are all sourced from the OTP read above into an `otp_data` buffer, then sliced by `init_sequence`. Register offsets into `otp_data`, byte counts, and even which registers are OTP-derived vs. hardcoded **differ per variant** — always re-derive from the matching `case` block in `Pervasive_BWRY_Small::COG_initial()`, never assume one variant's layout applies to another:
  - **`DriverF`** (`eScreen_EPD_154_QS_0F`, shared with `213_QS_0F`/`266_QS_0F`; e.g. `E2154QS0F1`): chip ID `0x0302`, 48-byte OTP read, no bank-2 fallback (fails immediately if the `0xA5` marker isn't found). `0x30` (PLL) is a **fixed** `0x08`, not OTP-derived — a rare exception to the "everything comes from OTP" rule for this family. Also uses three registers (`0x4d`, `0xb4`, `0xb5`) not present in `DriverA`'s layout at all.
  - **`DriverA`** (`eScreen_EPD_417_QS_0A`; e.g. `E2417QS0A3`): chip ID `0x0605`, 112-byte OTP read, with a bank-2 fallback (skip 111 bytes, re-check for the `0xA5` marker at the fallback offset) if the primary marker read fails.
  - Do not confuse this `DriverF` (BWRY, Spectra-4, chip ID `0x0302`) with `PervasiveDriverVariant::DriverF` in the sibling `pervasive_bw.rs` module (BW/BWR, chip-ID-less, hardcoded `0x4D`/`0xE9` registers) — same vendor naming reused across two unrelated COG families.

### 12. Single-Plane Frame Format
- **Rule**: Frame data is one packed 2-bits-per-pixel buffer written entirely via `WRITE_DATA = 0x10` — there is no split `WRITE_BW_DATA`/`WRITE_RED_DATA` plane pair like the BW/BWR controller's `0x10`/`0x13`.

### 13. DriverA Power-On Timing
- **Rule**: `DriverA` sends `POWER_ON (0x04)` as the last step of its OTP-derived main init block (`init_sequence`); `DriverF` defers `POWER_ON` to `trigger_refresh`. A parity check must confirm `POWER_ON` appears exactly once per full init→refresh cycle, at the correct stage for the variant in use.

### 14. Chip-ID Normalization
- **Rule**: A raw OTP chip-ID response of `0x8302` must be normalized to `0x0302` before comparing against the variant's expected ID (`0x0302` for `DriverF`, `0x0605` for `DriverA`) — a quirk carried over directly from the C++ reference's chip-ID check. Note `DriverF`'s expected ID and the normalized value are the same (`0x0302`), so a raw `0x8302` response and a raw `0x0302` response are both valid, indistinguishable-after-normalization matches for `DriverF`.

### 15. Extension Board J3 Jumper Configuration (EXT3-1)
- **Rule** (applies to both families — hardware-board-level, not COG-specific):
  - Small displays ($\le 3.7"$, e.g., 2.9" `E2290KS0F1`, 1.54" `E2154QS0F1`): **J3 Jumper OPEN** ($10\,\mu\text{H}$ inductor path).
  - Large displays ($> 3.7"$, e.g., 4.2" `E2417QS0A3`, 9.7"): **J3 Jumper CLOSED** ($47\,\mu\text{H}$ inductor path).
- **Troubleshooting**: If J3 is closed when driving a small display, DC-DC boost converter power sags during refresh, causing initialization failure or BUSY pin hangs.

---

## Verification Workflow

1. **Verify panel identity** against the vendor's own product page and datasheet — see "Panel Identification Verification" above. Do this first; every later step assumes you're auditing against the correct C++ case block.
2. **Check C++ Reference Code**: Compare registers, timings, and byte streams against `Pervasive_Wide_Small.cpp` (BW/BWR) or `Pervasive_BWRY_Small.cpp` (BWRY), whichever family you're auditing.
3. **Run Automated Tests**:
   ```bash
   cargo test --test pervasive_tests       # BW/BWR (PervasiveBwController)
   cargo test --test pervasive_bwry_tests  # BWRY (PervasiveBwryController)
   ```
4. **Verify SPI Traces**: Ensure the mock unit tests pass and confirm exact byte sequence order.
   - `tests/pervasive_tests.rs` uses the standard `RecordingSpiBus` (`SpiDevice`-based, command/data only).
   - `tests/pervasive_bwry_tests.rs` has **two** independent test doubles: a `RecordingSpiBus` (same as above) for the normal `SpiBusWrapper`-based `EpdController` methods (`init_sequence`, `write_frame`, `trigger_refresh`, `sleep`), and a separate bit-level `MockState`/`MockCs`/`MockSck`/`MockData`/`MockDc`/`MockRst`/`MockBusyIdle` set (implementing `Spi3Bus`'s pin traits, including `DynamicPin`) that reconstructs whole bytes from the individual bit-level `set_as_output`/`set_high`/`set_low`/`set_as_input`/`is_high` calls `Spi3Bus::write_byte`/`read_byte` make, used to test `read_otp` — a plain `SpiDevice`-based mock cannot exercise this path at all, since `read_otp` never touches `SpiDevice`.
5. **Real hardware caveat**: passing steps 2–4 only proves internal consistency, not that the correct panel variant was selected (that's what step 1 is for). If real hardware is available, the OTP chip-ID returned by the physical panel is the ultimate ground truth for which `PervasiveBwryVariant` applies — trust it over any assumption made in steps 1–4, and re-run step 1 against the *reported* chip ID if it disagrees with the expected one.

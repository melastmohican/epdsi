#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.
//!
//! `Jd79660Controller` is a thin wrapper over `Jd7966xController`'s `Jd7966xVariant::Jd79660`
//! arm — see `tests/jd79661_tests.rs` for the sibling IC sharing the same shared controller.
//!
//! JD79660AA's busy pin is active-low (busy while LOW), so `FixedPin(true)` — not `DummyPin`,
//! which reads low — must back BUSY, or `wait_busy(false)` spins forever.

use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_jd79660_gdem0154f51h_panel_dimensions() {
    assert_eq!(GDEM0154F51H::WIDTH, 200);
    assert_eq!(GDEM0154F51H::HEIGHT, 200);
    assert_eq!(GxEPD2_154c_GDEM0154F51H::WIDTH, 200);
    assert_eq!(GxEPD2_154c_GDEM0154F51H::HEIGHT, 200);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79660_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();

    // Confirmed byte-for-byte identical in Waveshare's `EPD_1IN54G_Init()` and GxEPD2's
    // `_InitDisplay()` (plain, non-fast path) — cross-checked against the JD79660A datasheet
    // (v1.0.3) for register lengths/defaults. `hard_reset`/`wait_busy` only touch the RST/BUSY
    // pins, so the first SPI record is the MAGIC_KEY write, not a reset command byte.
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x4D), // MAGIC_KEY (undocumented in either public datasheet)
            SpiRecord::Data(vec![0x78]),
            SpiRecord::Command(0x00), // PANEL_SETTING — 0x0F, not JD79661's 0x8F
            SpiRecord::Data(vec![0x0F, 0x29]),
            SpiRecord::Command(0x06), // BOOSTER_SOFT_START
            SpiRecord::Data(vec![0x0D, 0x12, 0x30, 0x20, 0x19, 0x2A, 0x22]),
            SpiRecord::Command(0x50), // CDI
            SpiRecord::Data(vec![0x37]),
            SpiRecord::Command(0x61), // RESOLUTION — 200, 200 (already 8px-aligned)
            SpiRecord::Data(vec![0x00, 0xC8, 0x00, 0xC8]),
            SpiRecord::Command(0xE9), // undocumented in either public datasheet
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x30), // PLL_CONTROL
            SpiRecord::Data(vec![0x08]),
            SpiRecord::Command(0x04), // POWER_ON, bare
        ]
    );
}
epd_test!(test_jd79660_init_sequence, jd79660_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79660_write_frame_ignores_channel_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .await
        .unwrap();
    let black_white = bus_backend.records.borrow().clone();
    assert_eq!(
        black_white,
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xAA, 0xBB])]
    );

    // Whole packed 2bpp buffer goes out in one transaction via DATA_START_TRANSMISSION — the
    // channel argument carries no separate-plane meaning here, unlike Pervasive's BWRY controller.
    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xAA, 0xBB])
        .await
        .unwrap();
    assert_eq!(bus_backend.records.borrow().clone(), black_white);
}
epd_test!(
    test_jd79660_write_frame_ignores_channel,
    jd79660_write_frame_ignores_channel_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79660_write_frame_pattern_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);

    // 200 * 200 / 4 px-per-byte = 10000 bytes for a full clear. `send_data_repeated` chunks in
    // 64-byte pieces internally, so `coalesce` flattens them into one comparable buffer.
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0x55, 10_000)
        .await
        .unwrap();
    let records = coalesce(&bus_backend.records.borrow());

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0x55; 10_000]),
        ]
    );
}
epd_test!(
    test_jd79660_write_frame_pattern,
    jd79660_write_frame_pattern_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79660_trigger_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    // Both datasheets document DISPLAY_REFRESH (0x12) as taking one mandatory data byte,
    // default 0x00 — confirmed by Waveshare's `EPD_1IN54G_TurnOnDisplay()` and GxEPD2's
    // `_refresh()`. Unlike `Jd79661Controller` (see `tests/jd79661_tests.rs`), which used to
    // send this bare — do not "simplify" this back to a bare command.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x12), SpiRecord::Data(vec![0x00])]
    );
}
epd_test!(test_jd79660_trigger_refresh, jd79660_trigger_refresh_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79660_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).await.unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x02), // POWER_OFF, + mandatory 0x00 data byte per datasheet
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x07), // DEEP_SLEEP
            SpiRecord::Data(vec![0xA5]),
        ]
    );
}
epd_test!(test_jd79660_sleep, jd79660_sleep_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79660_clear_frame_uses_the_correct_2bpp_byte_count_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEM0154F51H>::new(controller).build(bus);
    let mut delay = DummyDelay;

    driver.init(&mut delay).await.unwrap();
    bus_backend.records.borrow_mut().clear();

    // Locks in the `EpdDriver::clear_frame`/`RAM_WIDTH` alignment-aware fix: 200 (already
    // byte-aligned, no RAM_WIDTH override) * 2 bits/pixel / 8 * 200 rows = 10000 bytes.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .unwrap();
    let records = coalesce(&bus_backend.records.borrow());

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0xFF; 10_000]),
        ]
    );
}
epd_test!(
    test_jd79660_clear_frame_uses_the_correct_2bpp_byte_count,
    jd79660_clear_frame_uses_the_correct_2bpp_byte_count_body
);

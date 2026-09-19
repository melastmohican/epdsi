#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.
//!
//! `Jd79661Controller` is a thin wrapper over `Jd7966xController`'s `Jd7966xVariant::Jd79661`
//! arm — see `tests/jd79660_tests.rs` for the sibling IC sharing the same shared controller. No
//! exact-byte pinning existed for this controller before this file (its only prior coverage was
//! an untyped smoke test in `tests/compile_tests.rs`) — this is also what locks in the
//! `trigger_refresh`/`sleep` datasheet-corroborated bug fix (both used to send their command
//! bare, with no trailing data byte).
//!
//! JD79661's busy pin is active-low (busy while LOW), so `FixedPin(true)` — not `DummyPin`,
//! which reads low — must back BUSY, or `wait_busy(false)` spins forever.

use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_jd79661_zjy122250_panel_dimensions() {
    assert_eq!(ZJY122250_0213AJH_E5::WIDTH, 122);
    assert_eq!(ZJY122250_0213AJH_E5::HEIGHT, 250);
    assert_eq!(GDEY0213F51::WIDTH, 122);
    assert_eq!(GDEY0213F51::HEIGHT, 250);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79661_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller =
        Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();

    // Unchanged from before the `Jd7966xController` restructure — `hard_reset`/`wait_busy` only
    // touch the RST/BUSY pins, so the first SPI record is the initial POWER_SETTING probe.
    assert_eq!(
        records,
        vec![
            // POWER_SETTING probe before the real config below — `send_data` short-circuits on
            // an empty slice, so no `Data` record follows this `Command`.
            SpiRecord::Command(0x01),
            SpiRecord::Command(0x4D), // MAGIC_KEY (undocumented in either public datasheet)
            SpiRecord::Data(vec![0x78]),
            SpiRecord::Command(0x00), // PANEL_SETTING — 0x8F, not JD79660's 0x0F
            SpiRecord::Data(vec![0x8F, 0x29]),
            SpiRecord::Command(0x01), // POWER_SETTING, real config
            SpiRecord::Data(vec![0x07, 0x00]),
            SpiRecord::Command(0x03), // POWER_OFFSET
            SpiRecord::Data(vec![0x10, 0x54, 0x44]),
            SpiRecord::Command(0x06), // BOOSTER_SOFT_START
            SpiRecord::Data(vec![0x05, 0x00, 0x3F, 0x0A, 0x25, 0x12, 0x1A]),
            SpiRecord::Command(0x50), // CDI
            SpiRecord::Data(vec![0x37]),
            SpiRecord::Command(0x60), // TCON
            SpiRecord::Data(vec![0x02, 0x02]),
            SpiRecord::Command(0x61), // RESOLUTION — 122px RAM-padded to 128
            SpiRecord::Data(vec![0x00, 0x80, 0x00, 0xFA]),
            SpiRecord::Command(0xE7), // undocumented in either public datasheet
            SpiRecord::Data(vec![0x1C]),
            SpiRecord::Command(0xE3),
            SpiRecord::Data(vec![0x22]),
            SpiRecord::Command(0xB4),
            SpiRecord::Data(vec![0xD0]),
            SpiRecord::Command(0xB5),
            SpiRecord::Data(vec![0x03]),
            SpiRecord::Command(0xE9),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x30), // PLL_CONTROL (renamed from VCOM_CONTROL)
            SpiRecord::Data(vec![0x08]),
            SpiRecord::Command(0x04), // POWER_ON, bare
        ]
    );
}
epd_test!(test_jd79661_init_sequence, jd79661_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79661_write_frame_ignores_channel_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller =
        Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .await
        .unwrap();
    let black_white = bus_backend.records.borrow().clone();
    assert_eq!(
        black_white,
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xAA, 0xBB])]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::Yellow, &[0xAA, 0xBB])
        .await
        .unwrap();
    assert_eq!(bus_backend.records.borrow().clone(), black_white);
}
epd_test!(
    test_jd79661_write_frame_ignores_channel,
    jd79661_write_frame_ignores_channel_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79661_write_frame_pattern_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller =
        Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);

    // 128px RAM-padded width * 2 bits/pixel / 8 * 250 rows = 8000 bytes — the byte count the
    // `EpdDriver::clear_frame`/`RAM_WIDTH` alignment fix now also derives for this panel.
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0x55, 8_000)
        .await
        .unwrap();
    let records = coalesce(&bus_backend.records.borrow());

    assert_eq!(
        records,
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x55; 8_000])]
    );
}
epd_test!(
    test_jd79661_write_frame_pattern,
    jd79661_write_frame_pattern_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79661_trigger_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller =
        Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);
    let mut delay = DummyDelay;

    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    // Bug fix, corroborated by both the JD79660A and JD79661AA datasheets (R12H: "1 data byte,
    // default 00h") and Adafruit's own `Adafruit_JD79661::update()`. This used to be a bare
    // `Command(0x12)` with no data byte — do not revert to that shape.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x12), SpiRecord::Data(vec![0x00])]
    );
}
epd_test!(test_jd79661_trigger_refresh, jd79661_trigger_refresh_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79661_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller =
        Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).await.unwrap();

    // Bug fix, corroborated the same way as `trigger_refresh` above (R02H: "1 data byte,
    // default 00h"). Used to be a bare `Command(0x02)`.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x02),
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xA5]),
        ]
    );
}
epd_test!(test_jd79661_sleep, jd79661_sleep_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79661_clear_frame_uses_the_correct_2bpp_byte_count_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller =
        Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);
    let mut driver = EpdBuilder::<_, ZJY122250_0213AJH_E5>::new(controller).build(bus);
    let mut delay = DummyDelay;

    driver.init(&mut delay).await.unwrap();
    bus_backend.records.borrow_mut().clear();

    // Locks in the `EpdDriver::clear_frame`/`RAM_WIDTH` alignment-aware fix: this panel's
    // RAM_WIDTH override (128, padded from its 122px visible WIDTH) * 2 bits/pixel / 8 * 250
    // rows = 8000 bytes — previously miscomputed as 4000 (plain `WIDTH.div_ceil(8) * HEIGHT`).
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .unwrap();
    let records = coalesce(&bus_backend.records.borrow());

    assert_eq!(
        records,
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0xFF; 8_000])]
    );
}
epd_test!(
    test_jd79661_clear_frame_uses_the_correct_2bpp_byte_count,
    jd79661_clear_frame_uses_the_correct_2bpp_byte_count_body
);

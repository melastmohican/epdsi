#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async), see `tests/support/mod.rs`.
//!
//! Every expected byte stream below is Good Display's own `AU-GDEW0102I4FC-20250926` demo.

use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_uc8175_gdew0102i4fc_panel_dimensions() {
    assert_eq!(GDEW0102I4FC::WIDTH, 80);
    assert_eq!(GDEW0102I4FC::HEIGHT, 128);
    // 80 / 8 = 10 bytes per row, 128 rows: the vendor's EPD_ARRAY.
    assert_eq!(GDEW0102I4FC::WIDTH.div_ceil(8) * GDEW0102I4FC::HEIGHT, 1280);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8175_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8175Controller::new();
    let mut delay = RecordingDelay::new();

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // `EPD_Init`: PSR `5F`, hidden `2A 00 00`, PON, CDI `97`.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x5F]),
            SpiRecord::Command(0x2A),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x97]),
        ]
    );
    // One reset pulse: 5 ms high, 10 ms low, 10 ms high.
    assert_eq!(delay.calls_ms, vec![5, 10, 10]);
}
epd_test!(test_uc8175_init_sequence, uc8175_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8175_rotated_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8175Controller::new().with_rotated_180(true);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    // `EPD_Init_180`: identical to `EPD_Init` except for PSR `53`.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x00),
            SpiRecord::Data(vec![0x53]),
            SpiRecord::Command(0x2A),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x04),
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0x97]),
        ]
    );
}
epd_test!(
    test_uc8175_rotated_init_sequence,
    uc8175_rotated_init_sequence_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8175_full_frame_write_and_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8175Controller::new();
    let mut delay = RecordingDelay::new();

    // `EPD_WhiteScreen_ALL`: old plane `0x10` first, then new plane `0x13`, then `0x12`.
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xFF, 0xFF])
        .await
        .unwrap();
    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xA5, 0x5A])
        .await
        .unwrap();
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0xFF, 0xFF]),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0xA5, 0x5A]),
            SpiRecord::Command(0x12),
        ]
    );
    // The demo's necessary 1 ms gap, then a wait for BUSY to assert.
    assert_eq!(delay.calls_ms.first(), Some(&1));
    #[cfg(feature = "blocking")]
    assert_eq!(delay.calls_ms.len(), 1 + 500);
    #[cfg(not(feature = "blocking"))]
    assert_eq!(delay.calls_ms, vec![1]);
}
epd_test!(
    test_uc8175_full_frame_write_and_refresh,
    uc8175_full_frame_write_and_refresh_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8175_pattern_write_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8175Controller::new();

    // `EPD_WhiteScreen_White`: both planes filled with `0xFF`.
    controller
        .write_frame_pattern(&mut bus, ColorChannel::RedYellow, 0xFF, 3)
        .await
        .unwrap();
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 3)
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x10),
            SpiRecord::Data(vec![0xFF; 3]),
            SpiRecord::Command(0x13),
            SpiRecord::Data(vec![0xFF; 3]),
        ]
    );
}
epd_test!(test_uc8175_pattern_write, uc8175_pattern_write_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8175_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Uc8175Controller::new();
    let mut delay = RecordingDelay::new();

    controller.sleep(&mut bus, &mut delay).await.unwrap();

    // `EPD_DeepSleep`: CDI `F7`, POF, 100 ms, DSLP `A5`.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x50),
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x02),
            SpiRecord::Command(0x07),
            SpiRecord::Data(vec![0xA5]),
        ]
    );
    assert_eq!(delay.calls_ms, vec![100]);
}
epd_test!(test_uc8175_sleep, uc8175_sleep_body);

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.

//! Byte-pinned tests for the SSD1683 variant of the SSD168x controller, paired with the
//! `GDEY042T81` panel. Each expected stream is derived from Good Display's
//! `AU-GDEY042T81-FP4G-20240801` demo and the SSD1683 datasheet (Rev 1.0), with the deliberate
//! departures (data entry `0x03`, `0x18 80` at init, no hard reset before a partial) called out
//! where they appear.

use epdsi::controllers::ssd168x::{Ssd168xController, Ssd168xRefreshMode, Ssd168xVariant};
use epdsi::panels::GDEY042T81;
use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_ssd1683_gdey042t81_panel_dimensions() {
    assert_eq!(GDEY042T81::WIDTH, 400);
    assert_eq!(GDEY042T81::HEIGHT, 300);
    assert_eq!(GDEY042T81::COLOR_MODE, ColorMode::BlackWhite);
}

#[test]
fn test_ssd1683_constructors_select_the_variant() {
    let a = Ssd168xController::new_ssd1683(400, 300);
    let b = Ssd168xController::for_panel::<GDEY042T81>(Ssd168xVariant::Ssd1683);
    assert_eq!(a.variant(), Ssd168xVariant::Ssd1683);
    assert_eq!(b.variant(), Ssd168xVariant::Ssd1683);
}

fn controller() -> Ssd168xController {
    Ssd168xController::for_panel::<GDEY042T81>(Ssd168xVariant::Ssd1683)
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn record_refresh(mode: Ssd168xRefreshMode) -> Vec<SpiRecord> {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = controller().with_refresh_mode(mode);
    let mut delay = DummyDelay;
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();
    records
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1683_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = controller();
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x12), // SW_RESET
            SpiRecord::Command(0x01), // DRIVER_CONTROL: gate lines 300, so 299 = 0x012B, scan 0x00
            SpiRecord::Data(vec![0x2B, 0x01, 0x00]),
            SpiRecord::Command(0x3C), // BORDER_WAVEFORM_CONTROL
            SpiRecord::Data(vec![0x05]),
            SpiRecord::Command(0x21), // DISPLAY_UPDATE_CTRL1: bypass red RAM as zero
            SpiRecord::Data(vec![0x40, 0x00]),
            SpiRecord::Command(0x18), // TEMP_CONTROL: internal sensor (the vendor demo omits it)
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x11), // DATA_ENTRY_MODE: X increment, Y increment (vendor: 0x01)
            SpiRecord::Data(vec![0x03]),
            SpiRecord::Command(0x44), // SET_RAMXPOS: 400 px is 50 bytes, so 0..=49
            SpiRecord::Data(vec![0x00, 0x31]),
            SpiRecord::Command(0x45), // SET_RAMYPOS: 0..=299
            SpiRecord::Data(vec![0x00, 0x00, 0x2B, 0x01]),
            SpiRecord::Command(0x4E), // SET_RAMXCNT
            SpiRecord::Data(vec![0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT
            SpiRecord::Data(vec![0x00, 0x00]),
        ]
    );
}
epd_test!(test_ssd1683_init_sequence, ssd1683_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1683_full_refresh_body() {
    // Vendor `EPD_Update`: 0xF7, preceded by the `0x21 = 40 00` its init always sets.
    assert_eq!(
        record_refresh(Ssd168xRefreshMode::Full).await,
        vec![
            SpiRecord::Command(0x21), // restores the init value after a Partial or Gray4Otp
            SpiRecord::Data(vec![0x40, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(test_ssd1683_full_refresh, ssd1683_full_refresh_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1683_partial_refresh_body() {
    // Vendor `EPD_Dis_Part`: border 0x80 and 0x21 = 00 00, then 0xFF (GxEPD2 and ESPHome send
    // 0xFC). The vendor's leading hard reset is not sent, and there is no 0xE0/0x83 envelope.
    assert_eq!(
        record_refresh(Ssd168xRefreshMode::Partial).await,
        vec![
            SpiRecord::Command(0x3C),
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xFF]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(test_ssd1683_partial_refresh, ssd1683_partial_refresh_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1683_fast_full_refresh_body() {
    // Vendor `EPD_HW_Init_Fast` then `EPD_Update_Fast`: 0x1A = 6E (a single data byte), load LUT on 0x91, display on
    // 0xC7. The leading 0xB1 is the crate's shared temperature-load step.
    assert_eq!(
        record_refresh(Ssd168xRefreshMode::FastFull).await,
        vec![
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xB1]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x1A),
            SpiRecord::Data(vec![0x6E]), // one byte: the SSD1683 register is 8 bits wide
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0x91]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x40, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xC7]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(
    test_ssd1683_fast_full_refresh,
    ssd1683_fast_full_refresh_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1683_sleep_has_100ms_settle_delay_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = controller();
    let mut delay = RecordingDelay::new();

    controller.sleep(&mut bus, &mut delay).await.unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x01])]
    );
    assert_eq!(delay.calls_ms, vec![100]);
}
epd_test!(
    test_ssd1683_sleep_has_100ms_settle_delay,
    ssd1683_sleep_has_100ms_settle_delay_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1683_gray4_otp_refresh_body() {
    // Vendor `EPD_HW_Init_4G` then `EPD_Update_4G`: 0x21 = 00 00 (red RAM live), 0x1A = 5A, load
    // the OTP waveform on 0x91, display on 0xCF. No LUT upload and no 0xB1 preamble.
    assert_eq!(
        record_refresh(Ssd168xRefreshMode::Gray4Otp).await,
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x1A),
            SpiRecord::Data(vec![0x5A]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0x91]),
            SpiRecord::Command(0x20),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xCF]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(
    test_ssd1683_gray4_otp_refresh,
    ssd1683_gray4_otp_refresh_body
);

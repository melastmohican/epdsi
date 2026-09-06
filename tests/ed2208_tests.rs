#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.

use epdsi::prelude::*;

mod support;
use support::*;

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ed2208_gdep073e01_instantiation_and_rendering_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    // Active-low BUSY: `true` (high) means idle.
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEP073E01>::new(controller).build(bus);

    assert_eq!(driver.width(), 800);
    assert_eq!(driver.height(), 480);

    driver.init(&mut delay).await.expect("ED2208 init failed");

    let white_packed = SevenColor::pack(SevenColor::White, SevenColor::White);
    assert_eq!(white_packed, 0x11);
    driver
        .clear_frame(ColorChannel::Color7(0), white_packed)
        .await
        .expect("ED2208 clear frame failed");

    driver.refresh(&mut delay).await.expect("ED2208 refresh failed");
    driver.sleep(&mut delay).await.expect("ED2208 sleep failed");
}

epd_test!(
    test_ed2208_gdep073e01_instantiation_and_rendering,
    ed2208_gdep073e01_instantiation_and_rendering_body
);

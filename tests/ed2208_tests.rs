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
    let records_before_clear = bus_backend.records.borrow().len();
    driver
        .clear_frame(ColorChannel::Color7(0), white_packed)
        .await
        .expect("ED2208 clear frame failed");

    // GDEP073E01 is SevenColor (4bpp, 2 pixels/byte): the correct byte count is
    // width*height/2, not the 1-bit-per-pixel width.div_ceil(8)*height a non-SevenColor panel
    // would use. Regression guard for the under-clear bug where clear_frame fell through to the
    // 1bpp formula and only wrote 1/4 of the plane.
    let clear_records = bus_backend.records.borrow()[records_before_clear..].to_vec();
    let coalesced = coalesce(&clear_records);
    assert_eq!(coalesced, vec![
        SpiRecord::Command(0x10), // DATA_START_TRANSMISSION
        SpiRecord::Data(vec![white_packed; 800 * 480 / 2]),
    ]);

    driver.refresh(&mut delay).await.expect("ED2208 refresh failed");
    driver.sleep(&mut delay).await.expect("ED2208 sleep failed");
}

epd_test!(
    test_ed2208_gdep073e01_instantiation_and_rendering,
    ed2208_gdep073e01_instantiation_and_rendering_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ed2208_narrowed_window_widens_back_out_on_refresh_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEP073E01>::new(controller).build(bus);

    driver.init(&mut delay).await.expect("ED2208 init failed");

    driver
        .set_window(0, 0, 99, 99)
        .await
        .expect("ED2208 set_window failed");

    // Regression guard: a narrowed `set_window` must not leak into a subsequent full-panel
    // refresh. `trigger_refresh` must reassert the full-panel PARTIAL_WINDOW (0x83) before
    // sending DISPLAY_REFRESH (0x12), the same way GxEPD2 reasserts it before every operation.
    let records_before_refresh = bus_backend.records.borrow().len();
    driver.refresh(&mut delay).await.expect("ED2208 refresh failed");

    let refresh_records = bus_backend.records.borrow()[records_before_refresh..].to_vec();
    let coalesced = coalesce(&refresh_records);
    assert_eq!(
        coalesced,
        vec![
            SpiRecord::Command(0x83), // PARTIAL_WINDOW, reasserted full-panel bounds
            SpiRecord::Data(vec![0x00, 0x00, 0x03, 0x1F, 0x00, 0x00, 0x01, 0xE0, 0x01]),
            SpiRecord::Command(0x12), // DISPLAY_REFRESH
            SpiRecord::Data(vec![0x00]),
        ]
    );
}

epd_test!(
    test_ed2208_narrowed_window_widens_back_out_on_refresh,
    ed2208_narrowed_window_widens_back_out_on_refresh_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ed2208_de_ghost_clean_sweeps_then_refreshes_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEP073E01>::new(controller).build(bus);

    driver.init(&mut delay).await.expect("ED2208 init failed");

    let clean_packed = SevenColor::pack(SevenColor::Clean, SevenColor::Clean);
    assert_eq!(clean_packed, 0x77);

    let records_before = bus_backend.records.borrow().len();
    driver.de_ghost(&mut delay).await.expect("ED2208 de_ghost failed");

    let records = bus_backend.records.borrow()[records_before..].to_vec();
    let coalesced = coalesce(&records);
    assert_eq!(
        coalesced,
        vec![
            SpiRecord::Command(0x10), // DATA_START_TRANSMISSION
            SpiRecord::Data(vec![clean_packed; 800 * 480 / 2]),
            SpiRecord::Command(0x83), // PARTIAL_WINDOW, reasserted full-panel bounds
            SpiRecord::Data(vec![0x00, 0x00, 0x03, 0x1F, 0x00, 0x00, 0x01, 0xE0, 0x01]),
            SpiRecord::Command(0x12), // DISPLAY_REFRESH
            SpiRecord::Data(vec![0x00]),
        ]
    );
}

epd_test!(
    test_ed2208_de_ghost_clean_sweeps_then_refreshes,
    ed2208_de_ghost_clean_sweeps_then_refreshes_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ed2208_trigger_partial_refresh_narrows_without_widening_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .expect("ED2208 init failed");

    let records_before = bus_backend.records.borrow().len();
    controller
        .trigger_partial_refresh(&mut bus, &mut delay, 100, 50, 200, 80)
        .await
        .expect("trigger_partial_refresh failed");

    let records = bus_backend.records.borrow()[records_before..].to_vec();
    let coalesced = coalesce(&records);
    assert_eq!(
        coalesced,
        vec![
            SpiRecord::Command(0x83), // PARTIAL_WINDOW: caller's actual window, not widened
            SpiRecord::Data(vec![0x00, 100, 0x01, 43, 0x00, 50, 0x00, 130, 0x01]),
            SpiRecord::Command(0x12), // DISPLAY_REFRESH
            SpiRecord::Data(vec![0x00]),
            // No CDI (0x50) write: CDI is left untouched, matching Good Display's own
            // EPD_refresh() and Zephyr's ed2208_gca driver, neither of which resend it per
            // refresh with a different byte for a partial update.
        ]
    );
}

epd_test!(
    test_ed2208_trigger_partial_refresh_narrows_without_widening,
    ed2208_trigger_partial_refresh_narrows_without_widening_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ed2208_trigger_partial_refresh_rejects_odd_alignment_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);
    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .expect("ED2208 init failed");

    // Odd x.
    let err = controller
        .trigger_partial_refresh(&mut bus, &mut delay, 1, 0, 200, 80)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        EpdBusError::InvalidPartialWindowAlignment { x: 1, width: 200 }
    );

    // Odd width.
    let err = controller
        .trigger_partial_refresh(&mut bus, &mut delay, 0, 0, 201, 80)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        EpdBusError::InvalidPartialWindowAlignment { x: 0, width: 201 }
    );

    // Zero width/height.
    let err = controller
        .trigger_partial_refresh(&mut bus, &mut delay, 0, 0, 0, 80)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        EpdBusError::InvalidPartialWindowAlignment { x: 0, width: 0 }
    );
}

epd_test!(
    test_ed2208_trigger_partial_refresh_rejects_odd_alignment,
    ed2208_trigger_partial_refresh_rejects_odd_alignment_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ed2208_trigger_partial_refresh_rejects_out_of_bounds_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let mut controller = Ed2208Controller::new(GDEP073E01::WIDTH, GDEP073E01::HEIGHT);
    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .expect("ED2208 init failed");

    let err = controller
        .trigger_partial_refresh(&mut bus, &mut delay, 700, 0, 200, 80)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        EpdBusError::InvalidWindow {
            x_start: 700,
            y_start: 0,
            x_end: 899,
            y_end: 79,
            panel_width: 800,
            panel_height: 480,
        }
    );
}

epd_test!(
    test_ed2208_trigger_partial_refresh_rejects_out_of_bounds,
    ed2208_trigger_partial_refresh_rejects_out_of_bounds_body
);

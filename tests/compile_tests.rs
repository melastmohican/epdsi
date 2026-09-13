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
async fn ssd1681_epd_driver_instantiation_and_paged_rendering_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ssd1681Controller::new(GDEM0154Z90::WIDTH, GDEM0154Z90::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEM0154Z90>::new(controller).build(bus);

    assert_eq!(driver.width(), 200);
    assert_eq!(driver.height(), 200);

    driver
        .init(&mut delay)
        .await
        .expect("Initialization failed");

    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("Clear frame failed");

    let mut page_buffer = [0u8; (200 * 20) / 8];
    render_paged(
        &mut driver,
        &mut delay,
        ColorChannel::BlackWhite,
        &mut page_buffer,
        20,
        0xFF,
        |page_buf| {
            page_buf.set_pixel(10, page_buf.y_offset() + 5, true);
        },
    )
    .await
    .expect("Paged rendering failed");
}
epd_test!(
    test_ssd1681_epd_driver_instantiation_and_paged_rendering,
    ssd1681_epd_driver_instantiation_and_paged_rendering_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn uc8253_se0352n14_epd_driver_instantiation_and_paged_rendering_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    // Active-low BUSY: `true` (high) means idle.
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Uc8253Controller::new(SE0352N14TNGA0::WIDTH, SE0352N14TNGA0::HEIGHT)
        .with_variant(Uc8253Variant::Se0352n14);
    let mut driver = EpdBuilder::<_, SE0352N14TNGA0>::new(controller).build(bus);

    assert_eq!(driver.width(), 240);
    assert_eq!(driver.height(), 360);

    driver
        .init(&mut delay)
        .await
        .expect("Initialization failed");

    // Both planes clear to 0x00 on this panel: set bits are ink, not the mono convention.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0x00)
        .await
        .expect("Clear black/white frame failed");
    driver
        .clear_frame(ColorChannel::RedYellow, 0x00)
        .await
        .expect("Clear red frame failed");

    // 360 divides evenly into 20-row pages; 30 bytes per line.
    let mut page_buffer = [0u8; (240 * 20) / 8];
    render_paged(
        &mut driver,
        &mut delay,
        ColorChannel::BlackWhite,
        &mut page_buffer,
        20,
        0x00,
        |page_buf| {
            page_buf.set_pixel(10, page_buf.y_offset() + 5, true);
        },
    )
    .await
    .expect("Paged rendering failed");
}
epd_test!(
    test_uc8253_se0352n14_epd_driver_instantiation_and_paged_rendering,
    uc8253_se0352n14_epd_driver_instantiation_and_paged_rendering_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1680_gdey0266z90_epd_driver_instantiation_and_paged_rendering_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ssd1680Controller::new(GDEY0266Z90::WIDTH, GDEY0266Z90::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEY0266Z90>::new(controller).build(bus);

    assert_eq!(driver.width(), 152);
    assert_eq!(driver.height(), 296);

    driver
        .init(&mut delay)
        .await
        .expect("Initialization failed");

    // The Red plane is inverted relative to the Black/White plane: 0xFF is white in 0x24, but
    // 0x00 is *no* red in 0x26.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("Clear black/white frame failed");
    driver
        .clear_frame(ColorChannel::RedYellow, 0x00)
        .await
        .expect("Clear red frame failed");

    // 296 divides evenly into 8-row pages; 19 bytes per line.
    let mut page_buffer = [0u8; (152 * 8) / 8];
    render_paged(
        &mut driver,
        &mut delay,
        ColorChannel::BlackWhite,
        &mut page_buffer,
        8,
        0xFF,
        |page_buf| {
            page_buf.set_pixel(10, page_buf.y_offset() + 5, true);
        },
    )
    .await
    .expect("Paged rendering failed");
}
epd_test!(
    test_ssd1680_gdey0266z90_epd_driver_instantiation_and_paged_rendering,
    ssd1680_gdey0266z90_epd_driver_instantiation_and_paged_rendering_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1680_gdey0266z90_tri_color_paged_rendering_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ssd1680Controller::new(GDEY0266Z90::WIDTH, GDEY0266Z90::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEY0266Z90>::new(controller).build(bus);

    driver
        .init(&mut delay)
        .await
        .expect("Initialization failed");

    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("Clear black/white frame failed");
    driver
        .clear_frame(ColorChannel::RedYellow, 0x00)
        .await
        .expect("Clear red frame failed");

    // 296 divides evenly into 8-row pages; 19 bytes per line, one buffer per plane.
    let mut bw_page_buffer = [0u8; (152 * 8) / 8];
    let mut accent_page_buffer = [0u8; (152 * 8) / 8];
    render_paged_tri_color(
        &mut driver,
        &mut delay,
        ColorChannel::RedYellow,
        (&mut bw_page_buffer, &mut accent_page_buffer),
        PlanePolarity::SSD168X,
        8,
        |page_buf| {
            let y = page_buf.bw().y_offset() + 2;
            page_buf.set_pixel(10, y, TriColor::Black);
            page_buf.set_pixel(11, y, TriColor::Accent);
            page_buf.set_pixel(12, y, TriColor::White);
        },
    )
    .await
    .expect("Tri-Color paged rendering failed");
}
epd_test!(
    test_ssd1680_gdey0266z90_tri_color_paged_rendering,
    ssd1680_gdey0266z90_tri_color_paged_rendering_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1680_gdey0266t90_gray4_paged_rendering_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Ssd1680Controller::for_panel::<GDEY0266T90>()
        .with_gray4(GDEY0266T90::GRAY4)
        .with_refresh_mode(Ssd168xRefreshMode::Gray4);
    let mut driver = EpdBuilder::<_, GDEY0266T90>::new(controller).build(bus);

    driver
        .init(&mut delay)
        .await
        .expect("Initialization failed");

    // 152 is byte-aligned: 19 bytes per line, one buffer per plane, 8-row page.
    let mut plane_a_page_buffer = [0u8; (152 / 8) * 8];
    let mut plane_b_page_buffer = [0u8; (152 / 8) * 8];
    render_paged_gray4(
        &mut driver,
        &mut delay,
        (&mut plane_a_page_buffer, &mut plane_b_page_buffer),
        Gray4Polarity::ADAFRUIT_SSD1680,
        8,
        |page_buf| {
            let y = page_buf.plane_a().y_offset() + 2;
            page_buf.set_pixel(10, y, Gray4Color::White);
            page_buf.set_pixel(11, y, Gray4Color::Light);
            page_buf.set_pixel(12, y, Gray4Color::Dark);
            page_buf.set_pixel(13, y, Gray4Color::Black);
        },
    )
    .await
    .expect("Gray4 paged rendering failed");
}
epd_test!(
    test_ssd1680_gdey0266t90_gray4_paged_rendering,
    ssd1680_gdey0266t90_gray4_paged_rendering_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79661_epd_driver_instantiation_and_frame_write_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller =
        Jd79661Controller::new(ZJY122250_0213AJH_E5::WIDTH, ZJY122250_0213AJH_E5::HEIGHT);
    let mut driver = EpdBuilder::<_, ZJY122250_0213AJH_E5>::new(controller).build(bus);

    assert_eq!(driver.width(), 122);
    assert_eq!(driver.height(), 250);

    driver
        .init(&mut delay)
        .await
        .expect("Initialization failed");

    // Quad-Color panels pack 2 bits/pixel and aren't wired into the 1bpp `render_paged` sweep
    // (see `RAM_WIDTH`/`quad_color_row_bytes` in `src/driver.rs`) — drive `clear_frame`/
    // `write_frame` directly instead, at the panel's real 2bpp RAM size: 128px RAM_WIDTH (this
    // panel pads 122px to 128px) * 2 bits/pixel / 8 * 250 rows = 8000 bytes.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("Clear frame failed");

    let frame = [0x55u8; 8000];
    driver
        .write_frame(ColorChannel::BlackWhite, &frame)
        .await
        .expect("Write frame failed");
}
epd_test!(
    test_jd79661_epd_driver_instantiation_and_frame_write,
    jd79661_epd_driver_instantiation_and_frame_write_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn jd79660_epd_driver_instantiation_and_frame_write_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = Jd79660Controller::new(GDEM0154F51H::WIDTH, GDEM0154F51H::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEM0154F51H>::new(controller).build(bus);

    assert_eq!(driver.width(), 200);
    assert_eq!(driver.height(), 200);

    driver
        .init(&mut delay)
        .await
        .expect("Initialization failed");

    // 200 is already 8px-aligned, so RAM_WIDTH stays at its default (200): 200 * 2 bits/pixel / 8
    // * 200 rows = 10000 bytes — this is what locks in the `EpdDriver::clear_frame`/
    // `required_bytes` alignment-aware fix for a byte-aligned QuadColor panel.
    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("Clear frame failed");

    let frame = [0x55u8; 10_000];
    driver
        .write_frame(ColorChannel::BlackWhite, &frame)
        .await
        .expect("Write frame failed");
}
epd_test!(
    test_jd79660_epd_driver_instantiation_and_frame_write,
    jd79660_epd_driver_instantiation_and_frame_write_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn pervasive_e2266ks0c1_epd_driver_instantiation_and_paged_rendering_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut delay = DummyDelay;

    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, FixedPin(true));
    let controller = PervasiveBwController::new(E2266KS0C1::WIDTH, E2266KS0C1::HEIGHT);
    let mut driver = EpdBuilder::<_, E2266KS0C1>::new(controller).build(bus);

    assert_eq!(driver.width(), 152);
    assert_eq!(driver.height(), 296);

    driver
        .init(&mut delay)
        .await
        .expect("Initialization failed");

    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .expect("Clear frame failed");

    let mut page_buffer = [0u8; (152 * 8) / 8];
    render_paged(
        &mut driver,
        &mut delay,
        ColorChannel::BlackWhite,
        &mut page_buffer,
        8,
        0xFF,
        |page_buf| {
            page_buf.set_pixel(10, page_buf.y_offset() + 2, true);
        },
    )
    .await
    .expect("Paged rendering failed");

    driver.refresh(&mut delay).await.expect("Refresh failed");
    driver.sleep(&mut delay).await.expect("Sleep failed");
}
epd_test!(
    test_pervasive_e2266ks0c1_epd_driver_instantiation_and_paged_rendering,
    pervasive_e2266ks0c1_epd_driver_instantiation_and_paged_rendering_body
);

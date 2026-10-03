#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only. Dual-mode: runs under both `cargo test` (blocking) and
//! `cargo test --no-default-features --features graphics` (async) — see `tests/support/mod.rs`.

use epdsi::prelude::*;

mod support;
use support::*;

#[test]
fn test_ssd1677_gdeq0426t82_panel_dimensions() {
    assert_eq!(GDEQ0426T82::WIDTH, 800);
    assert_eq!(GDEQ0426T82::HEIGHT, 480);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();

    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x12), // SW_RESET
            SpiRecord::Command(0x18), // TEMP_CONTROL
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x0C), // BOOSTER_SOFT_START
            SpiRecord::Data(vec![0xAE, 0xC7, 0xC3, 0xC0, 0x80]),
            SpiRecord::Command(0x01), // DRIVER_CONTROL
            SpiRecord::Data(vec![0xDF, 0x01, 0x02]),
            SpiRecord::Command(0x3C), // BORDER_WAVEFORM_CONTROL
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x11), // DATA_ENTRY_MODE (X+, Y-), asserted by set_window
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x44), // SET_RAMXPOS: 16-bit pixel start/end, 0..=799 (0x031F)
            SpiRecord::Data(vec![0x00, 0x00, 0x1F, 0x03]),
            SpiRecord::Command(0x45), // SET_RAMYPOS (end pair first, reversed)
            SpiRecord::Data(vec![0xDF, 0x01, 0x00, 0x00]),
            SpiRecord::Command(0x4E), // SET_RAMXCNT: 16-bit pixel counter
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT
            SpiRecord::Data(vec![0xDF, 0x01]),
        ]
    );
}
epd_test!(test_ssd1677_init_sequence, ssd1677_init_sequence_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_set_window_sub_rectangle_y_reversal_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    // x_start=100 rounds down to pixel 96 (0x0060), x_end=199 rounds up to pixel 199 (0x00C7),
    // y_start=50, y_end=99 (h=50). yy = 480-50-50 = 380 = 0x017C, yy_end = 480-50-1 = 429 = 0x01AD.
    controller
        .set_window(&mut bus, 100, 50, 199, 99)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x11),
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x44),
            SpiRecord::Data(vec![0x60, 0x00, 0xC7, 0x00]),
            SpiRecord::Command(0x45),
            SpiRecord::Data(vec![0xAD, 0x01, 0x7C, 0x01]),
        ]
    );

    bus_backend.records.borrow_mut().clear();
    controller.set_cursor(&mut bus, 100, 50).await.unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x4E),
            SpiRecord::Data(vec![0x60, 0x00]),
            SpiRecord::Command(0x4F),
            SpiRecord::Data(vec![0xAD, 0x01]),
        ]
    );
}
epd_test!(
    test_ssd1677_set_window_sub_rectangle_y_reversal,
    ssd1677_set_window_sub_rectangle_y_reversal_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_ram_x_registers_are_16_bit_and_pixel_valued_body() {
    // Property test rather than a byte-for-byte pin: the SSD1677 X address is wider and in
    // different units than the SSD1680/SSD1681 registers this controller was adapted from.
    // Short-writing `SET_RAMXPOS` leaves the end address unset, which scrambles the frame.
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .set_window(
            &mut bus,
            0,
            0,
            GDEQ0426T82::WIDTH - 1,
            GDEQ0426T82::HEIGHT - 1,
        )
        .await
        .unwrap();
    controller.set_cursor(&mut bus, 0, 0).await.unwrap();

    let records = bus_backend.records.borrow().clone();
    let payload_after = |command: u8| -> Vec<u8> {
        let idx = records
            .iter()
            .position(|r| *r == SpiRecord::Command(command))
            .unwrap_or_else(|| panic!("command {:#04X} never sent", command));
        match &records[idx + 1] {
            SpiRecord::Data(d) => d.clone(),
            other => panic!("expected data after {:#04X}, got {:?}", command, other),
        }
    };

    let xpos = payload_after(0x44);
    assert_eq!(xpos.len(), 4, "SET_RAMXPOS takes a 16-bit start and end");
    let x_start = u16::from(xpos[0]) | (u16::from(xpos[1]) << 8);
    let x_end = u16::from(xpos[2]) | (u16::from(xpos[3]) << 8);
    assert_eq!(x_start, 0);
    assert_eq!(
        x_end,
        (GDEQ0426T82::WIDTH - 1) as u16,
        "end address is a pixel index, not a byte index"
    );

    let xcnt = payload_after(0x4E);
    assert_eq!(xcnt.len(), 2, "SET_RAMXCNT takes a 16-bit counter");
    assert_eq!(u16::from(xcnt[0]) | (u16::from(xcnt[1]) << 8), 0);
}
epd_test!(
    test_ssd1677_ram_x_registers_are_16_bit_and_pixel_valued,
    ssd1677_ram_x_registers_are_16_bit_and_pixel_valued_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_write_frame_channel_routing_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x24), SpiRecord::Data(vec![0xAA, 0xBB]),]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xCC])
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x26), SpiRecord::Data(vec![0xCC])]
    );
}
epd_test!(
    test_ssd1677_write_frame_channel_routing,
    ssd1677_write_frame_channel_routing_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_trigger_refresh_all_modes_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // Full (default)
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    assert_eq!(controller.refresh_mode(), Ssd1677RefreshMode::Full);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x40, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x20),
        ]
    );

    // FastFull
    bus_backend.records.borrow_mut().clear();
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_refresh_mode(Ssd1677RefreshMode::FastFull);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x40, 0x00]),
            SpiRecord::Command(0x1A),
            SpiRecord::Data(vec![0x5A]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xD7]),
            SpiRecord::Command(0x20),
        ]
    );

    // Partial
    bus_backend.records.borrow_mut().clear();
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_refresh_mode(Ssd1677RefreshMode::Partial);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xFC]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(
    test_ssd1677_trigger_refresh_all_modes,
    ssd1677_trigger_refresh_all_modes_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_sleep_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).await.unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x01])]
    );
}
epd_test!(test_ssd1677_sleep, ssd1677_sleep_body);

// --- The clear path: streaming and auto-fill -------------------------------------------------
//
// `write_frame_pattern` is what `clear_frame` funnels into. Since 0.1.7 it has two paths: the
// controller's own RAM pattern generator (`0x46`/`0x47`) for a uniform full-plane fill, and the
// original `send_data_repeated` stream for everything the generator cannot express. A botched
// auto-fill renders as a partly-cleared panel rather than an error, so the boundary between the
// two paths is asserted here rather than left to hardware to discover.

/// Flattens the recorded stream into `(command, payload)` pairs, concatenating the 64-byte
/// chunks `send_data_repeated` emits so a 48,000-byte clear is assertable.
fn coalesce(records: &[SpiRecord]) -> Vec<(u8, Vec<u8>)> {
    let mut out: Vec<(u8, Vec<u8>)> = Vec::new();
    for record in records {
        match record {
            SpiRecord::Command(c) => out.push((*c, Vec::new())),
            SpiRecord::Data(d) => match out.last_mut() {
                Some((_, payload)) => payload.extend_from_slice(d),
                None => panic!("data {:?} arrived before any command", d),
            },
        }
    }
    out
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_write_frame_pattern_streams_the_fill_byte_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    // 150 is deliberately not a multiple of the 64-byte chunk, so a short final chunk is covered.
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 150)
        .await
        .unwrap();

    let records = bus_backend.records.borrow().clone();
    // Today: one command plus 3 chunks (64 + 64 + 22).
    assert_eq!(
        records.len(),
        4,
        "expected WRITE_BW_DATA plus three streamed chunks, got {:?}",
        records
            .iter()
            .map(std::mem::discriminant)
            .collect::<Vec<_>>()
    );
    assert_eq!(records[0], SpiRecord::Command(0x24));
    assert_eq!(records[1], SpiRecord::Data(vec![0xFF; 64]));
    assert_eq!(records[2], SpiRecord::Data(vec![0xFF; 64]));
    assert_eq!(records[3], SpiRecord::Data(vec![0xFF; 22]));

    let coalesced = coalesce(&records);
    assert_eq!(coalesced.len(), 1);
    assert_eq!(coalesced[0].0, 0x24);
    assert_eq!(coalesced[0].1, vec![0xFF; 150]);
}
epd_test!(
    test_ssd1677_write_frame_pattern_streams_the_fill_byte,
    ssd1677_write_frame_pattern_streams_the_fill_byte_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_write_frame_pattern_channel_routing_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0x00, 4)
        .await
        .unwrap();
    controller
        .write_frame_pattern(&mut bus, ColorChannel::RedYellow, 0xFF, 4)
        .await
        .unwrap();

    assert_eq!(
        coalesce(&bus_backend.records.borrow()),
        vec![(0x24, vec![0x00; 4]), (0x26, vec![0xFF; 4])],
        "BlackWhite routes to WRITE_BW_DATA, every colour channel to WRITE_RED_DATA"
    );
}
epd_test!(
    test_ssd1677_write_frame_pattern_channel_routing,
    ssd1677_write_frame_pattern_channel_routing_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_write_frame_pattern_zero_count_still_selects_the_plane_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 0)
        .await
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x24)],
        "a zero-length fill emits the plane-select command and no data"
    );
}
epd_test!(
    test_ssd1677_write_frame_pattern_zero_count_still_selects_the_plane,
    ssd1677_write_frame_pattern_zero_count_still_selects_the_plane_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_clear_frame_uses_the_auto_fill_registers_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEQ0426T82>::new(controller).build(bus);

    driver
        .clear_frame(ColorChannel::BlackWhite, 0xFF)
        .await
        .unwrap();
    driver
        .clear_frame(ColorChannel::RedYellow, 0x00)
        .await
        .unwrap();

    // 0xF7: A[7]=1 first step value, A[6:4]=111 step height 680 gates, A[2:0]=111 step width
    // 960 sources. Both steps span the 800 x 480 panel, so the pattern never alternates inside
    // it and the plane comes out uniform. 0x77 is the same with a zero first step.
    //
    // Each sweep is followed by a cursor re-seat to the window origin. Streaming a plane left
    // the counter wrapped back there on its own, so without this a caller that wrote image data
    // straight after a clear — as the hardware examples do — would render it displaced.
    // 6 bytes per plane in place of 48,000.
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x47), // AUTO_WRITE_BW_RAM
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x4E), // SET_RAMXCNT, back to the window origin
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT: y=0 maps to RAM 479 (0x01DF), Y reversed
            SpiRecord::Data(vec![0xDF, 0x01]),
            SpiRecord::Command(0x46), // AUTO_WRITE_RED_RAM
            SpiRecord::Data(vec![0x77]),
            SpiRecord::Command(0x4E),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x4F),
            SpiRecord::Data(vec![0xDF, 0x01]),
        ]
    );
}
epd_test!(
    test_ssd1677_clear_frame_uses_the_auto_fill_registers,
    ssd1677_clear_frame_uses_the_auto_fill_registers_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_auto_fill_restores_the_cursor_to_a_narrowed_window_body() {
    // The re-seat must follow the window actually in force, not assume the full frame.
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .set_window(&mut bus, 100, 50, 199, 99)
        .await
        .unwrap();
    bus_backend.records.borrow_mut().clear();

    // A full-plane count still auto-fills; the sweep paints the RAM area, which is the window.
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 100 * 480)
        .await
        .unwrap();

    let records = bus_backend.records.borrow().clone();
    assert_eq!(records[0], SpiRecord::Command(0x47));
    // Same cursor bytes `set_cursor(100, 50)` emits on its own: x rounds to pixel 96 (0x0060),
    // y=50 maps to RAM 429 (0x01AD).
    assert_eq!(records[2], SpiRecord::Command(0x4E));
    assert_eq!(records[3], SpiRecord::Data(vec![0x60, 0x00]));
    assert_eq!(records[4], SpiRecord::Command(0x4F));
    assert_eq!(records[5], SpiRecord::Data(vec![0xAD, 0x01]));
}
epd_test!(
    test_ssd1677_auto_fill_restores_the_cursor_to_a_narrowed_window,
    ssd1677_auto_fill_restores_the_cursor_to_a_narrowed_window_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_auto_fill_falls_back_for_fills_it_cannot_express_body() {
    let full = 100 * 480;
    let cases: [(u8, usize, &str); 3] = [
        (0xAA, full, "a non-uniform byte is not a regular pattern"),
        (0xFF, full - 1, "a partial fill would paint the whole plane"),
        (0x00, 64, "a partial fill would paint the whole plane"),
    ];

    for (byte, count, why) in cases {
        let bus_backend = RecordingSpiBus::new();
        let dc = TestDc(&bus_backend);
        let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
        let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

        controller
            .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, byte, count)
            .await
            .unwrap();

        let coalesced = coalesce(&bus_backend.records.borrow());
        assert_eq!(coalesced.len(), 1);
        assert_eq!(coalesced[0].0, 0x24, "{why}: must stream, not auto-fill");
        assert_eq!(coalesced[0].1, vec![byte; count], "{why}");
    }
}
epd_test!(
    test_ssd1677_auto_fill_falls_back_for_fills_it_cannot_express,
    ssd1677_auto_fill_falls_back_for_fills_it_cannot_express_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_auto_fill_declines_panels_larger_than_the_maximum_step_body() {
    // A[2:0] tops out at 960 sources and A[6:4] at 680 gates. A panel past either would
    // alternate part-way across and clear to a half-inverted frame rather than failing, so the
    // controller must decline instead. No such SSD1677 panel ships today; this pins the guard
    // before one does.
    for (width, height) in [(1024u32, 480u32), (800, 720)] {
        let bus_backend = RecordingSpiBus::new();
        let dc = TestDc(&bus_backend);
        let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
        let mut controller = Ssd1677Controller::new(width, height);
        let count = width.div_ceil(8) as usize * height as usize;

        controller
            .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, count)
            .await
            .unwrap();

        let records = bus_backend.records.borrow().clone();
        assert_eq!(
            records[0],
            SpiRecord::Command(0x24),
            "{width}x{height} exceeds the pattern generator's reach and must stream"
        );
    }
}
epd_test!(
    test_ssd1677_auto_fill_declines_panels_larger_than_the_maximum_step,
    ssd1677_auto_fill_declines_panels_larger_than_the_maximum_step_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_auto_fill_can_be_switched_off_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller =
        Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT).with_ram_auto_fill(false);
    assert!(!controller.ram_auto_fill());

    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 100 * 480)
        .await
        .unwrap();

    let coalesced = coalesce(&bus_backend.records.borrow());
    assert_eq!(coalesced[0].0, 0x24);
    assert_eq!(coalesced[0].1.len(), 100 * 480);
}
epd_test!(
    test_ssd1677_auto_fill_can_be_switched_off,
    ssd1677_auto_fill_can_be_switched_off_body
);

// --- Panel config foundation (plan item 2d) --------------------------------------------------

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn record_ssd1677_init(controller: Ssd1677Controller) -> Vec<SpiRecord> {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = controller;
    let mut delay = DummyDelay;
    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();
    records
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_for_panel_is_byte_identical_to_the_hand_wired_form_body() {
    assert_eq!(
        record_ssd1677_init(Ssd1677Controller::for_panel::<GDEQ0426T82>()).await,
        record_ssd1677_init(Ssd1677Controller::new(
            GDEQ0426T82::WIDTH,
            GDEQ0426T82::HEIGHT
        ))
        .await,
    );
}
epd_test!(
    test_ssd1677_for_panel_is_byte_identical_to_the_hand_wired_form,
    ssd1677_for_panel_is_byte_identical_to_the_hand_wired_form_body
);

// --- Gray4 (Phase 4): two-pass grayscale, ported from `Adafruit_SSD1677::update()` -------------

#[test]
fn test_ssd1677_gdeq0426t82_gray4_lut_is_105_bytes() {
    let gray4 = GDEQ0426T82::GRAY4.unwrap();
    assert_eq!(
        gray4.lut.len(),
        105,
        "SSD1677's Gray4 LUT is shorter than SSD168x's 233 bytes"
    );
    assert_eq!(gray4.gate_voltage, 0x17);
    assert_eq!(gray4.source_voltage, [0x41, 0xA8, 0x32]);
    assert_eq!(gray4.vcom, 0x30);
    assert_eq!(gray4.border_waveform, 0x01);
}

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_gray4_init_sequence_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_gray4(GDEQ0426T82::GRAY4);
    let mut delay = DummyDelay;
    let gray4 = GDEQ0426T82::GRAY4.unwrap();

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();

    // Gray4's analog/waveform block (LUT + gate/source/VCOM voltages) entirely replaces the
    // plain path's VCOM/gate-voltage hook and custom-LUT tail, matching
    // `Adafruit_SSD1677::powerUp()`'s unconditional `_epd_init_code` + `_epd_lut_code` load. Note
    // the booster soft-start's last byte (0x40, not the plain path's 0x80) — ported directly from
    // `ti_426_gray4_init_code`, the crate's only Gray4 reference for this controller.
    assert_eq!(
        records,
        vec![
            SpiRecord::Command(0x12), // SW_RESET
            SpiRecord::Command(0x18), // TEMP_CONTROL
            SpiRecord::Data(vec![0x80]),
            SpiRecord::Command(0x0C), // BOOSTER_SOFT_START
            SpiRecord::Data(vec![0xAE, 0xC7, 0xC3, 0xC0, 0x40]),
            SpiRecord::Command(0x01), // DRIVER_CONTROL
            SpiRecord::Data(vec![0xDF, 0x01, 0x02]),
            SpiRecord::Command(0x3C), // BORDER_WAVEFORM_CONTROL
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x11), // DATA_ENTRY_MODE, asserted by set_window (unchanged by Gray4)
            SpiRecord::Data(vec![0x01]),
            SpiRecord::Command(0x44), // SET_RAMXPOS
            SpiRecord::Data(vec![0x00, 0x00, 0x1F, 0x03]),
            SpiRecord::Command(0x45), // SET_RAMYPOS
            SpiRecord::Data(vec![0xDF, 0x01, 0x00, 0x00]),
            SpiRecord::Command(0x4E), // SET_RAMXCNT
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x4F), // SET_RAMYCNT
            SpiRecord::Data(vec![0xDF, 0x01]),
            SpiRecord::Command(0x32), // WRITE_LUT_REGISTER — LUT first
            SpiRecord::Data(gray4.lut.to_vec()),
            SpiRecord::Command(0x03), // GATE_VOLTAGE — then voltages, per Adafruit's own ordering
            SpiRecord::Data(vec![0x17]),
            SpiRecord::Command(0x04), // SOURCE_VOLTAGE
            SpiRecord::Data(vec![0x41, 0xA8, 0x32]),
            SpiRecord::Command(0x2C), // WRITE_VCOM_REGISTER
            SpiRecord::Data(vec![0x30]),
        ]
    );
}
epd_test!(
    test_ssd1677_gray4_init_sequence,
    ssd1677_gray4_init_sequence_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_reload_gray4_lut_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let gray4 = GDEQ0426T82::GRAY4.unwrap();
    let controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_gray4(GDEQ0426T82::GRAY4);

    controller.reload_gray4_lut(&mut bus).await.unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x32),
            SpiRecord::Data(gray4.lut.to_vec()),
            SpiRecord::Command(0x03),
            SpiRecord::Data(vec![0x17]),
            SpiRecord::Command(0x04),
            SpiRecord::Data(vec![0x41, 0xA8, 0x32]),
            SpiRecord::Command(0x2C),
            SpiRecord::Data(vec![0x30]),
        ]
    );

    // No Gray4 configured: a no-op, not an error and not a partial write.
    bus_backend.records.borrow_mut().clear();
    let plain_controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    plain_controller.reload_gray4_lut(&mut bus).await.unwrap();
    assert_eq!(bus_backend.records.borrow().clone(), vec![]);
}
epd_test!(test_ssd1677_reload_gray4_lut, ssd1677_reload_gray4_lut_body);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_trigger_refresh_gray4_preclear_and_gray4_body() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // Preclear pass: bypass Red/Yellow, OTP LUT (0xF7) — same mode byte as `Full`, different
    // `DISPLAY_UPDATE_CTRL1` bypass than `Gray4`'s final pass.
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_refresh_mode(Ssd1677RefreshMode::Gray4Preclear);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x40, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xF7]),
            SpiRecord::Command(0x20),
        ]
    );

    // Final pass: NORMAL (both planes), custom LUT full power cycle (0xCF).
    bus_backend.records.borrow_mut().clear();
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_refresh_mode(Ssd1677RefreshMode::Gray4);
    controller
        .trigger_refresh(&mut bus, &mut delay)
        .await
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![
            SpiRecord::Command(0x21),
            SpiRecord::Data(vec![0x00, 0x00]),
            SpiRecord::Command(0x22),
            SpiRecord::Data(vec![0xCF]),
            SpiRecord::Command(0x20),
        ]
    );
}
epd_test!(
    test_ssd1677_trigger_refresh_gray4_preclear_and_gray4,
    ssd1677_trigger_refresh_gray4_preclear_and_gray4_body
);

#[test]
fn test_ssd1677_gdeq0426t82_declares_no_vcom() {
    // Ruled 1 Sep 2026 against `GxEPD2_426_GDEQ0426T82.cpp`, which writes no 0x2C anywhere: this
    // panel runs on its OTP VCOM. The const staying `None` is what keeps `for_panel` from
    // introducing a divergence the hand-wired form never had.
    assert_eq!(GDEQ0426T82::VCOM, None);
    assert_eq!(GDEQ0426T82::GATE_VOLTAGE, None);
    assert_eq!(GDEQ0426T82::CUSTOM_LUT, None);

    let controller = Ssd1677Controller::for_panel::<GDEQ0426T82>();
    assert_eq!(controller.vcom(), None);
}

use epdsi::controllers::ssd1677::cmd;

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_custom_lut_of_the_right_length_reaches_the_wire_body() {
    // 105 bytes: `cmd::WRITE_LUT_LEN`'s datasheet-cited length (WS byte 0~104). Content is an
    // arbitrary placeholder: this exercises plumbing/ordering, not real waveform bytes.
    const LUT: &[u8] = &[0x5A; 105];
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_lut(Some(LUT));
    let mut delay = DummyDelay;

    controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap();
    let records = bus_backend.records.borrow().clone();
    assert_eq!(records.last(), Some(&SpiRecord::Data(vec![0x5A; 105])));
    assert_eq!(
        records[records.len() - 2],
        SpiRecord::Command(cmd::WRITE_LUT_REGISTER)
    );
}
epd_test!(
    test_ssd1677_custom_lut_of_the_right_length_reaches_the_wire,
    ssd1677_custom_lut_of_the_right_length_reaches_the_wire_body
);

#[maybe_async_cfg::maybe(
    sync(feature = "blocking", keep_self),
    async(not(feature = "blocking"), keep_self)
)]
async fn ssd1677_rejects_a_custom_lut_of_the_wrong_length_body() {
    // One byte short of `cmd::WRITE_LUT_LEN` (105): must be rejected before anything reaches
    // the wire, not silently sent as a malformed payload.
    let short_lut: &'static [u8] = &[0x5A; 104];
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT)
        .with_lut(Some(short_lut));

    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;
    let err = controller
        .init_sequence(&mut bus, &mut delay)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        EpdBusError::InvalidLutLength {
            expected: cmd::WRITE_LUT_LEN,
            provided: 104,
        }
    );
    assert!(!bus_backend
        .records
        .borrow()
        .contains(&SpiRecord::Command(cmd::WRITE_LUT_REGISTER)));
}
epd_test!(
    test_ssd1677_rejects_a_custom_lut_of_the_wrong_length,
    ssd1677_rejects_a_custom_lut_of_the_wrong_length_body
);

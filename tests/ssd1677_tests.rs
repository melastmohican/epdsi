#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Test assertions are allowed to panic; the deny-by-default policy in `Cargo.toml`
//! targets library code only.

use core::cell::RefCell;
use embedded_hal::delay::DelayNs;
use embedded_hal::digital::{ErrorType as DigitalErrorType, InputPin, OutputPin};
use embedded_hal::spi::{ErrorKind, ErrorType as SpiErrorType, Operation, SpiDevice};
use epdsi::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
enum SpiRecord {
    Command(u8),
    Data(Vec<u8>),
}

#[derive(Debug)]
struct RecordingSpiBus {
    records: RefCell<Vec<SpiRecord>>,
    dc_state: RefCell<bool>, // false = Low (Command), true = High (Data)
}

impl RecordingSpiBus {
    fn new() -> Self {
        Self {
            records: RefCell::new(Vec::new()),
            dc_state: RefCell::new(false),
        }
    }
}

impl SpiErrorType for &RecordingSpiBus {
    type Error = ErrorKind;
}

impl SpiDevice for &RecordingSpiBus {
    fn transaction(&mut self, _operations: &mut [Operation<'_, u8>]) -> Result<(), Self::Error> {
        Ok(())
    }

    fn write(&mut self, buf: &[u8]) -> Result<(), Self::Error> {
        let is_data = *self.dc_state.borrow();
        if is_data {
            self.records
                .borrow_mut()
                .push(SpiRecord::Data(buf.to_vec()));
        } else {
            for &byte in buf {
                self.records.borrow_mut().push(SpiRecord::Command(byte));
            }
        }
        Ok(())
    }
}

struct TestDc<'a>(&'a RecordingSpiBus);

impl DigitalErrorType for TestDc<'_> {
    type Error = core::convert::Infallible;
}

impl OutputPin for TestDc<'_> {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        *self.0.dc_state.borrow_mut() = false;
        Ok(())
    }

    fn set_high(&mut self) -> Result<(), Self::Error> {
        *self.0.dc_state.borrow_mut() = true;
        Ok(())
    }
}

#[derive(Debug)]
struct DummyPin;

impl DigitalErrorType for DummyPin {
    type Error = core::convert::Infallible;
}

impl OutputPin for DummyPin {
    fn set_low(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
    fn set_high(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

impl InputPin for DummyPin {
    fn is_high(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn is_low(&mut self) -> Result<bool, Self::Error> {
        Ok(true)
    }
}

struct DummyDelay;

impl DelayNs for DummyDelay {
    fn delay_ns(&mut self, _ns: u32) {}
    fn delay_us(&mut self, _us: u32) {}
    fn delay_ms(&mut self, _ms: u32) {}
}

#[test]
fn test_ssd1677_gdeq0426t82_panel_dimensions() {
    assert_eq!(GDEQ0426T82::WIDTH, 800);
    assert_eq!(GDEQ0426T82::HEIGHT, 480);
}

#[test]
fn test_ssd1677_init_sequence() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut delay = DummyDelay;

    controller.init_sequence(&mut bus, &mut delay).unwrap();
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

#[test]
fn test_ssd1677_set_window_sub_rectangle_y_reversal() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    // x_start=100 rounds down to pixel 96 (0x0060), x_end=199 rounds up to pixel 199 (0x00C7),
    // y_start=50, y_end=99 (h=50). yy = 480-50-50 = 380 = 0x017C, yy_end = 480-50-1 = 429 = 0x01AD.
    controller.set_window(&mut bus, 100, 50, 199, 99).unwrap();
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
    controller.set_cursor(&mut bus, 100, 50).unwrap();
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

#[test]
fn test_ssd1677_ram_x_registers_are_16_bit_and_pixel_valued() {
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
        .unwrap();
    controller.set_cursor(&mut bus, 0, 0).unwrap();

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

#[test]
fn test_ssd1677_write_frame_channel_routing() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .write_frame(&mut bus, ColorChannel::BlackWhite, &[0xAA, 0xBB])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x24), SpiRecord::Data(vec![0xAA, 0xBB]),]
    );

    bus_backend.records.borrow_mut().clear();
    controller
        .write_frame(&mut bus, ColorChannel::RedYellow, &[0xCC])
        .unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x26), SpiRecord::Data(vec![0xCC])]
    );
}

#[test]
fn test_ssd1677_trigger_refresh_all_modes() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut delay = DummyDelay;

    // Full (default)
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    assert_eq!(controller.refresh_mode(), Ssd1677RefreshMode::Full);
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
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
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
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
    controller.trigger_refresh(&mut bus, &mut delay).unwrap();
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

#[test]
fn test_ssd1677_sleep() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut delay = DummyDelay;

    controller.sleep(&mut bus, &mut delay).unwrap();
    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x10), SpiRecord::Data(vec![0x01])]
    );
}

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

#[test]
fn test_ssd1677_write_frame_pattern_streams_the_fill_byte() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    // 150 is deliberately not a multiple of the 64-byte chunk, so a short final chunk is covered.
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 150)
        .unwrap();

    let records = bus_backend.records.borrow().clone();
    // Today: one command plus 3 chunks (64 + 64 + 22).
    assert_eq!(
        records.len(),
        4,
        "expected WRITE_BW_DATA plus three streamed chunks, got {:?}",
        records.iter().map(std::mem::discriminant).collect::<Vec<_>>()
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

#[test]
fn test_ssd1677_write_frame_pattern_channel_routing() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0x00, 4)
        .unwrap();
    controller
        .write_frame_pattern(&mut bus, ColorChannel::RedYellow, 0xFF, 4)
        .unwrap();

    assert_eq!(
        coalesce(&bus_backend.records.borrow()),
        vec![(0x24, vec![0x00; 4]), (0x26, vec![0xFF; 4])],
        "BlackWhite routes to WRITE_BW_DATA, every colour channel to WRITE_RED_DATA"
    );
}

#[test]
fn test_ssd1677_write_frame_pattern_zero_count_still_selects_the_plane() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 0)
        .unwrap();

    assert_eq!(
        bus_backend.records.borrow().clone(),
        vec![SpiRecord::Command(0x24)],
        "a zero-length fill emits the plane-select command and no data"
    );
}

#[test]
fn test_ssd1677_clear_frame_uses_the_auto_fill_registers() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);
    let mut driver = EpdBuilder::<_, GDEQ0426T82>::new(controller).build(bus);

    driver.clear_frame(ColorChannel::BlackWhite, 0xFF).unwrap();
    driver.clear_frame(ColorChannel::RedYellow, 0x00).unwrap();

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

#[test]
fn test_ssd1677_auto_fill_restores_the_cursor_to_a_narrowed_window() {
    // The re-seat must follow the window actually in force, not assume the full frame.
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller = Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT);

    controller.set_window(&mut bus, 100, 50, 199, 99).unwrap();
    bus_backend.records.borrow_mut().clear();

    // A full-plane count still auto-fills; the sweep paints the RAM area, which is the window.
    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 100 * 480)
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

#[test]
fn test_ssd1677_auto_fill_falls_back_for_fills_it_cannot_express() {
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
            .unwrap();

        let coalesced = coalesce(&bus_backend.records.borrow());
        assert_eq!(coalesced.len(), 1);
        assert_eq!(coalesced[0].0, 0x24, "{why}: must stream, not auto-fill");
        assert_eq!(coalesced[0].1, vec![byte; count], "{why}");
    }
}

#[test]
fn test_ssd1677_auto_fill_declines_panels_larger_than_the_maximum_step() {
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
            .unwrap();

        let records = bus_backend.records.borrow().clone();
        assert_eq!(
            records[0],
            SpiRecord::Command(0x24),
            "{width}x{height} exceeds the pattern generator's reach and must stream"
        );
    }
}

#[test]
fn test_ssd1677_auto_fill_can_be_switched_off() {
    let bus_backend = RecordingSpiBus::new();
    let dc = TestDc(&bus_backend);
    let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
    let mut controller =
        Ssd1677Controller::new(GDEQ0426T82::WIDTH, GDEQ0426T82::HEIGHT).with_ram_auto_fill(false);
    assert!(!controller.ram_auto_fill());

    controller
        .write_frame_pattern(&mut bus, ColorChannel::BlackWhite, 0xFF, 100 * 480)
        .unwrap();

    let coalesced = coalesce(&bus_backend.records.borrow());
    assert_eq!(coalesced[0].0, 0x24);
    assert_eq!(coalesced[0].1.len(), 100 * 480);
}

// --- Panel config foundation (plan item 2d) --------------------------------------------------

#[test]
fn test_ssd1677_for_panel_is_byte_identical_to_the_hand_wired_form() {
    let record = |controller: Ssd1677Controller| {
        let bus_backend = RecordingSpiBus::new();
        let dc = TestDc(&bus_backend);
        let mut bus = SpiBusWrapper::new(&bus_backend, dc, DummyPin, DummyPin);
        let mut controller = controller;
        let mut delay = DummyDelay;
        controller.init_sequence(&mut bus, &mut delay).unwrap();
        let records = bus_backend.records.borrow().clone();
        records
    };

    assert_eq!(
        record(Ssd1677Controller::for_panel::<GDEQ0426T82>()),
        record(Ssd1677Controller::new(
            GDEQ0426T82::WIDTH,
            GDEQ0426T82::HEIGHT
        )),
    );
}

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

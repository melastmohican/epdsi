//! `core::error::Error` and `Display` for the public error enums.

use epdsi::bus::{EpdBusError, ValidationError};

fn assert_error<E: core::error::Error>() {}

#[test]
fn error_enums_implement_core_error() {
    assert_error::<ValidationError>();
    assert_error::<EpdBusError<(), (), (), ()>>();
}

#[test]
fn bus_error_display_names_the_failure() {
    let e: EpdBusError<(), (), (), ()> = EpdBusError::BufferTooSmall {
        required: 10,
        provided: 4,
    };
    assert_eq!(
        e.to_string(),
        "frame buffer too small: need 10 bytes, got 4"
    );
    let t: EpdBusError<(), (), (), ()> = EpdBusError::Timeout;
    assert_eq!(t.to_string(), "BUSY stayed asserted past the timeout");
}

#[test]
fn validation_error_display_matches_bus_error() {
    let v = ValidationError::InvalidLutLength {
        expected: 153,
        provided: 10,
    };
    let b: EpdBusError<(), (), (), ()> = v.into();
    assert_eq!(v.to_string(), b.to_string());
}

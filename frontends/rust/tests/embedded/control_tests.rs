use super::*;
use std::panic::catch_unwind;

fn panic_message(payload: Box<dyn std::any::Any + Send>) -> String {
    payload
        .downcast_ref::<String>()
        .cloned()
        .or_else(|| payload.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_default()
}

#[test]
fn raise_internal_error_panics_directly_outside_a_test_context() {
    let payload = catch_unwind(|| raise_internal_error(format_args!("boom: {}", 7))).unwrap_err();
    let msg = panic_message(payload);
    assert!(msg.contains("Internal error in hegel at "), "{msg}");
    assert!(msg.contains("boom: 7"), "{msg}");
    assert!(msg.contains("bug in hegel"), "{msg}");
}

#[test]
fn raise_internal_error_unwinds_as_a_typed_payload_inside_a_test_context() {
    let payload = with_test_context(|| {
        catch_unwind(|| raise_internal_error(format_args!("inner"))).unwrap_err()
    });
    let internal = payload
        .downcast_ref::<InternalError>()
        .expect("expected an InternalError control payload");
    assert!(internal.0.contains("inner"), "{}", internal.0);
}

#[test]
fn internal_assert_includes_the_condition_when_it_fails() {
    let value = 3;
    let payload = catch_unwind(|| hegel_internal_assert!(value == 4)).unwrap_err();
    let msg = panic_message(payload);
    assert!(
        msg.contains("internal assertion failed: value == 4"),
        "{msg}"
    );
}

#[test]
fn internal_assert_passes_silently() {
    hegel_internal_assert!(1 + 1 == 2);
    hegel_internal_assert!(1 + 1 == 2, "with a message {}", "argument");
}

#[test]
fn internal_unwrap_returns_the_value_of_a_some() {
    assert_eq!(hegel_internal_unwrap!(Some(3), "missing {}", "value"), 3);
}

#[test]
fn internal_unwrap_raises_an_internal_error_for_a_none() {
    let payload =
        catch_unwind(|| hegel_internal_unwrap!(None::<u8>, "missing {}", "value")).unwrap_err();
    let msg = panic_message(payload);
    assert!(msg.contains("Internal error in hegel at "), "{msg}");
    assert!(msg.contains("missing value"), "{msg}");
}

//! Compile-time enforcement of CLAUDE.md's 200-line cap on the protocol
//! files most prone to silent growth (large enums + their newtype
//! payloads). Adding a single variant or doc line can push these past
//! the limit unnoticed in a PR diff; this test fails the build instead.

const EVENT_PAYLOAD: &str = include_str!("../../src/event_payload.rs");
const EVENT_SUMMARY: &str = include_str!("../../src/event_summary.rs");
const CONTROL: &str = include_str!("../../src/control.rs");

const HARD_LIMIT: usize = 200;

fn assert_under_cap(name: &str, content: &str) {
    let lines = content.lines().count();
    assert!(
        lines <= HARD_LIMIT,
        "{name} is {lines} lines (limit: {HARD_LIMIT}). \
         If you added a variant, consider moving its payload into \
         event_summary.rs as a newtype struct instead of growing the enum."
    );
}

#[test]
fn event_payload_under_line_cap() {
    assert_under_cap("event_payload.rs", EVENT_PAYLOAD);
}

#[test]
fn event_summary_under_line_cap() {
    assert_under_cap("event_summary.rs", EVENT_SUMMARY);
}

#[test]
fn control_under_line_cap() {
    assert_under_cap("control.rs", CONTROL);
}

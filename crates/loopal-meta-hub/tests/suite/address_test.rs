//! Tests for QualifiedAddress parsing and formatting.

use loopal_protocol::QualifiedAddress;

#[test]
fn parse_local() {
    let addr = QualifiedAddress::parse("main");
    assert_eq!(addr.hub, None);
    assert_eq!(addr.agent, "main");
    assert_eq!(addr.to_string(), "main");
}

#[test]
fn parse_remote() {
    let addr = QualifiedAddress::parse("hub-west/researcher");
    assert_eq!(addr.hub.as_deref(), Some("hub-west"));
    assert_eq!(addr.agent, "researcher");
    assert_eq!(addr.to_string(), "hub-west/researcher");
}

#[test]
fn roundtrip() {
    let original = "code-hub/worker-3";
    let addr = QualifiedAddress::parse(original);
    assert_eq!(addr.to_string(), original);
}

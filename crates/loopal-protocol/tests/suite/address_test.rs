use loopal_protocol::QualifiedAddress;

#[test]
fn parse_local_address_has_no_hub() {
    let addr = QualifiedAddress::parse("researcher");
    assert!(addr.hub.is_empty());
    assert_eq!(addr.agent, "researcher");
    assert!(addr.is_local());
    assert_eq!(addr.to_string(), "researcher");
}

#[test]
fn parse_single_hub_address() {
    let addr = QualifiedAddress::parse("hub-a/researcher");
    assert_eq!(addr.hub, vec!["hub-a"]);
    assert_eq!(addr.agent, "researcher");
    assert!(addr.is_remote());
    assert_eq!(addr.to_string(), "hub-a/researcher");
}

#[test]
fn parse_multi_hub_supports_metahub_of_metahub() {
    let addr = QualifiedAddress::parse("mh-1/hub-A/agent");
    assert_eq!(addr.hub, vec!["mh-1", "hub-A"]);
    assert_eq!(addr.agent, "agent");
    assert_eq!(addr.next_hop(), Some("mh-1"));
    assert_eq!(addr.to_string(), "mh-1/hub-A/agent");
}

#[test]
fn parse_malformed_falls_back_to_local() {
    // Empty segments anywhere collapse to a local address verbatim.
    let addr = QualifiedAddress::parse("/researcher");
    assert!(addr.hub.is_empty());
    assert_eq!(addr.agent, "/researcher");

    let addr = QualifiedAddress::parse("hub-a/");
    assert!(addr.hub.is_empty());

    let addr = QualifiedAddress::parse("");
    assert!(addr.hub.is_empty());
    assert_eq!(addr.agent, "");
}

#[test]
fn constructors_produce_expected_shape() {
    let local = QualifiedAddress::local("main");
    assert!(local.is_local());
    assert_eq!(local.agent, "main");

    let remote = QualifiedAddress::remote(["hub-b"], "worker");
    assert!(remote.is_remote());
    assert_eq!(remote.to_string(), "hub-b/worker");

    let layered = QualifiedAddress::remote(["mh-1", "hub-A"], "agent");
    assert_eq!(layered.hub.len(), 2);
}

#[test]
fn snat_prepends_hub_at_front() {
    let mut addr = QualifiedAddress::local("agent");
    addr.prepend_hub("hub-A");
    assert_eq!(addr.hub, vec!["hub-A"]);

    addr.prepend_hub("mh-1");
    assert_eq!(addr.hub, vec!["mh-1", "hub-A"]);
    assert_eq!(addr.to_string(), "mh-1/hub-A/agent");
}

#[test]
fn dnat_pops_front_and_returns_consumed_name() {
    let mut addr = QualifiedAddress::remote(["mh-1", "hub-A"], "agent");
    assert_eq!(addr.pop_front_hub().as_deref(), Some("mh-1"));
    assert_eq!(addr.next_hop(), Some("hub-A"));
    assert_eq!(addr.pop_front_hub().as_deref(), Some("hub-A"));
    assert!(addr.is_local());
    // Idempotent on already-local addresses.
    assert!(addr.pop_front_hub().is_none());
}

#[test]
fn parse_display_roundtrip_preserves_path() {
    let cases = [
        "agent",
        "hub-a/agent",
        "mh-1/hub-A/agent",
        "a/b/c/d/e/agent",
    ];
    for s in cases {
        let addr = QualifiedAddress::parse(s);
        assert_eq!(addr.to_string(), s, "roundtrip failed for {s}");
    }
}

#[test]
fn from_str_impls_parse_to_qualified() {
    let from_str: QualifiedAddress = "hub-a/agent".into();
    let from_string: QualifiedAddress = String::from("hub-a/agent").into();
    let parsed = QualifiedAddress::parse("hub-a/agent");
    assert_eq!(from_str, parsed);
    assert_eq!(from_string, parsed);
}

#[test]
fn prepend_hub_if_local_is_idempotent_on_qualified() {
    // Conditional SNAT — used by event aggregation to avoid double-stamping
    // an address that already carries a hub path from upstream NAT.
    let mut already_qualified = QualifiedAddress::remote(["hub-A"], "alpha");
    already_qualified.prepend_hub_if_local("hub-B");
    assert_eq!(
        already_qualified,
        QualifiedAddress::remote(["hub-A"], "alpha"),
        "qualified address must not be re-stamped"
    );

    let mut local = QualifiedAddress::local("alpha");
    local.prepend_hub_if_local("hub-B");
    assert_eq!(local, QualifiedAddress::remote(["hub-B"], "alpha"));
}

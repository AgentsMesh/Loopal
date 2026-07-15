use super::*;

#[test]
fn alive_roundtrip_carries_version_transport_and_capability() {
    let line = DesktopHandshake::alive("0.6.3-dev", 42, Some(7), "127.0.0.1:9000", "secret-token");

    let encoded = line.encode();
    assert!(encoded.starts_with(DESKTOP_HANDSHAKE_PREFIX));
    assert_eq!(encoded.matches('\n').count(), 1);
    let wire_json: serde_json::Value = serde_json::from_str(
        encoded
            .trim_end()
            .strip_prefix(DESKTOP_HANDSHAKE_PREFIX)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        wire_json,
        serde_json::json!({
            "protocol_version": 1,
            "server_version": "0.6.3-dev",
            "pid": 42,
            "parent_pid": 7,
            "phase": "alive",
            "addr": "127.0.0.1:9000",
            "token": "secret-token",
            "transport": "tcp_jsonrpc_ndjson",
            "capabilities": ["hub_ui_v1", "workspace_v1"]
        })
    );
    assert_eq!(
        DesktopHandshake::parse(&encoded).unwrap(),
        Some(line.clone())
    );
    assert_eq!(line.protocol_version, DESKTOP_PROTOCOL_VERSION);
    assert_eq!(line.server_version, "0.6.3-dev");
    assert_eq!(line.parent_pid, Some(7));
    assert_eq!(
        line.event,
        DesktopHandshakeEvent::Alive {
            addr: "127.0.0.1:9000".into(),
            token: "secret-token".into(),
            transport: DESKTOP_TRANSPORT.into(),
            capabilities: vec![
                DESKTOP_CAPABILITY_HUB_UI.into(),
                DESKTOP_CAPABILITY_WORKSPACE.into(),
            ],
        }
    );
}

#[test]
fn ready_roundtrip_omits_absent_parent_pid() {
    let line = DesktopHandshake::ready("1.0.0", 8, None, "session-1");
    let encoded = line.encode();
    assert!(!encoded.contains("parent_pid"));
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(
            encoded
                .trim_end()
                .strip_prefix(DESKTOP_HANDSHAKE_PREFIX)
                .unwrap()
        )
        .unwrap(),
        serde_json::json!({
            "protocol_version": 1,
            "server_version": "1.0.0",
            "pid": 8,
            "phase": "ready",
            "session_id": "session-1"
        })
    );
    assert_eq!(DesktopHandshake::parse(&encoded).unwrap(), Some(line));
}

#[test]
fn session_created_roundtrip_uses_optional_event_prefix() {
    let line = DesktopHandshake::session_created("1.0.0", 8, Some(7), "session-1");
    let encoded = line.encode();
    assert!(encoded.starts_with(DESKTOP_EVENT_PREFIX));
    let wire_json: serde_json::Value = serde_json::from_str(
        encoded
            .trim_end()
            .strip_prefix(DESKTOP_EVENT_PREFIX)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        wire_json,
        serde_json::json!({
            "protocol_version": 1,
            "server_version": "1.0.0",
            "pid": 8,
            "parent_pid": 7,
            "phase": "session_created",
            "session_id": "session-1"
        })
    );
    assert_eq!(DesktopHandshake::parse(&encoded).unwrap(), Some(line));
}

#[test]
fn parser_rejects_phase_on_wrong_prefix() {
    let optional = DesktopHandshake::session_created("1.0.0", 8, None, "session-1")
        .encode()
        .replacen(DESKTOP_EVENT_PREFIX, DESKTOP_HANDSHAKE_PREFIX, 1);
    assert!(DesktopHandshake::parse(&optional).is_err());

    let core = DesktopHandshake::ready("1.0.0", 8, None, "session-1")
        .encode()
        .replacen(DESKTOP_HANDSHAKE_PREFIX, DESKTOP_EVENT_PREFIX, 1);
    assert!(DesktopHandshake::parse(&core).is_err());
}

#[test]
fn error_message_remains_one_physical_line() {
    let line = DesktopHandshake::error(
        "1.0.0",
        8,
        Some(7),
        "startup_failed",
        "first line\nsecond line",
    );
    let encoded = line.encode();
    assert_eq!(encoded.matches('\n').count(), 1);
    assert!(encoded.contains(r#"first line\nsecond line"#));
    assert_eq!(DesktopHandshake::parse(&encoded).unwrap(), Some(line));
}

#[test]
fn non_protocol_line_is_ignored() {
    assert_eq!(DesktopHandshake::parse("ordinary output\n").unwrap(), None);
}

#[test]
fn malformed_prefixed_json_is_rejected() {
    assert!(DesktopHandshake::parse("LOOPAL_DESKTOP {not-json}\n").is_err());
}

#[test]
fn parser_accepts_crlf() {
    let line = DesktopHandshake::ready("1.0.0", 8, Some(7), "session-1");
    let encoded = line.encode().replace('\n', "\r\n");
    assert_eq!(DesktopHandshake::parse(&encoded).unwrap(), Some(line));
}

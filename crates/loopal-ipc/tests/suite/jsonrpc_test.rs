use loopal_ipc::jsonrpc::{
    self, INTERNAL_ERROR, INVALID_REQUEST, IncomingMessage, METHOD_NOT_FOUND, PARSE_ERROR,
};

#[test]
fn parse_request() {
    let data = br#"{"jsonrpc":"2.0","id":1,"method":"test","params":{"key":"val"}}"#;
    let msg = jsonrpc::parse_message(data).expect("should parse");
    match msg {
        IncomingMessage::Request { id, method, params } => {
            assert_eq!(id, 1);
            assert_eq!(method, "test");
            assert_eq!(params["key"], "val");
        }
        _ => panic!("expected Request"),
    }
}

#[test]
fn parse_notification() {
    let data = br#"{"jsonrpc":"2.0","method":"event","params":{}}"#;
    let msg = jsonrpc::parse_message(data).expect("should parse");
    match msg {
        IncomingMessage::Notification { method, .. } => {
            assert_eq!(method, "event");
        }
        _ => panic!("expected Notification"),
    }
}

#[test]
fn parse_response_with_result() {
    let data = br#"{"jsonrpc":"2.0","id":42,"result":{"ok":true}}"#;
    let msg = jsonrpc::parse_message(data).expect("should parse");
    match msg {
        IncomingMessage::Response { id, result, error } => {
            assert_eq!(id, 42);
            assert!(result.is_some());
            assert!(error.is_none());
        }
        _ => panic!("expected Response"),
    }
}

#[test]
fn parse_response_with_error() {
    let data = br#"{"jsonrpc":"2.0","id":5,"error":{"code":-32600,"message":"bad request"}}"#;
    let msg = jsonrpc::parse_message(data).expect("should parse");
    match msg {
        IncomingMessage::Response { id, error, .. } => {
            assert_eq!(id, 5);
            let err = error.expect("should have error");
            assert_eq!(err.code, INVALID_REQUEST);
            assert_eq!(err.message, "bad request");
        }
        _ => panic!("expected Response"),
    }
}

#[test]
fn parse_malformed_returns_none() {
    assert!(jsonrpc::parse_message(b"not json").is_none());
    assert!(jsonrpc::parse_message(b"{}").is_none());
    // Non-numeric id
    assert!(
        jsonrpc::parse_message(br#"{"jsonrpc":"2.0","id":"str","method":"x","params":{}}"#)
            .is_none()
    );
}

#[test]
fn encode_request_roundtrip() {
    let data = jsonrpc::encode_request(7, "foo/bar", serde_json::json!({"a": 1}));
    let msg = jsonrpc::parse_message(&data).expect("should parse");
    match msg {
        IncomingMessage::Request { id, method, params } => {
            assert_eq!(id, 7);
            assert_eq!(method, "foo/bar");
            assert_eq!(params["a"], 1);
        }
        _ => panic!("expected Request"),
    }
}

#[test]
fn encode_notification_roundtrip() {
    let data = jsonrpc::encode_notification("ping", serde_json::json!(null));
    let msg = jsonrpc::parse_message(&data).expect("should parse");
    match msg {
        IncomingMessage::Notification { method, .. } => {
            assert_eq!(method, "ping");
        }
        _ => panic!("expected Notification"),
    }
}

#[test]
fn encode_response_roundtrip() {
    let data = jsonrpc::encode_response(99, serde_json::json!({"ok": true}));
    let msg = jsonrpc::parse_message(&data).expect("should parse");
    match msg {
        IncomingMessage::Response { id, result, .. } => {
            assert_eq!(id, 99);
            assert_eq!(result.unwrap()["ok"], true);
        }
        _ => panic!("expected Response"),
    }
}

#[test]
fn encode_error_roundtrip() {
    let data = jsonrpc::encode_error(3, INTERNAL_ERROR, "oops");
    let msg = jsonrpc::parse_message(&data).expect("should parse");
    match msg {
        IncomingMessage::Response { id, error, .. } => {
            assert_eq!(id, 3);
            let err = error.expect("should have error");
            assert_eq!(err.code, INTERNAL_ERROR);
            assert_eq!(err.message, "oops");
        }
        _ => panic!("expected Response"),
    }
}

#[test]
fn error_codes_match_spec() {
    assert_eq!(PARSE_ERROR, -32700);
    assert_eq!(INVALID_REQUEST, -32600);
    assert_eq!(METHOD_NOT_FOUND, -32601);
    assert_eq!(INTERNAL_ERROR, -32603);
}

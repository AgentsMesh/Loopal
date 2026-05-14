use loopal_tool_invocation::{InvocationId, InvocationIdError};

#[test]
fn empty_string_rejected() {
    assert_eq!(InvocationId::new(""), Err(InvocationIdError::Empty));
}

#[test]
fn non_empty_string_accepted() {
    let id = InvocationId::new("tc-1").unwrap();
    assert_eq!(id.as_str(), "tc-1");
}

#[test]
fn try_from_string_validates() {
    let s: String = "tc-2".to_string();
    let id = InvocationId::try_from(s).unwrap();
    assert_eq!(id.as_str(), "tc-2");
    assert!(InvocationId::try_from(String::new()).is_err());
}

#[test]
fn try_from_str_validates() {
    let id = InvocationId::try_from("tc-3").unwrap();
    assert_eq!(id.as_str(), "tc-3");
    assert!(InvocationId::try_from("").is_err());
}

#[test]
fn display_renders_inner() {
    let id = InvocationId::new("abc").unwrap();
    assert_eq!(format!("{id}"), "abc");
}

#[test]
fn as_ref_returns_str() {
    let id = InvocationId::new("xyz").unwrap();
    let s: &str = id.as_ref();
    assert_eq!(s, "xyz");
}

#[test]
fn into_inner_returns_owned() {
    let id = InvocationId::new("hello").unwrap();
    assert_eq!(id.into_inner(), "hello".to_string());
}

#[test]
fn equality_and_hash_work() {
    use std::collections::HashSet;
    let a = InvocationId::new("k").unwrap();
    let b = InvocationId::new("k").unwrap();
    assert_eq!(a, b);
    let mut set = HashSet::new();
    set.insert(a);
    assert!(set.contains(&b));
}

#[test]
fn deserialize_empty_string_rejected() {
    let res: Result<InvocationId, _> = serde_json::from_str("\"\"");
    assert!(
        res.is_err(),
        "empty string must not deserialize to InvocationId (invariant violation)"
    );
}

#[test]
fn deserialize_non_empty_string_accepted() {
    let id: InvocationId = serde_json::from_str("\"tc-42\"").unwrap();
    assert_eq!(id.as_str(), "tc-42");
}

#[test]
fn serialize_roundtrip() {
    let id = InvocationId::new("rt").unwrap();
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"rt\"");
    let back: InvocationId = serde_json::from_str(&json).unwrap();
    assert_eq!(back, id);
}

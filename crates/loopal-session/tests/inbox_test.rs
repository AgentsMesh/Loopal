//! Tests for Inbox queue.

use loopal_session::inbox::Inbox;

#[test]
fn test_inbox_new_is_empty() {
    let inbox = Inbox::new();
    assert!(inbox.is_empty());
    assert_eq!(inbox.len(), 0);
}

#[test]
fn test_push_and_pop_front() {
    let mut inbox = Inbox::new();
    inbox.push("first".to_string());
    inbox.push("second".to_string());
    assert_eq!(inbox.len(), 2);

    assert_eq!(inbox.pop_front(), Some("first".to_string()));
    assert_eq!(inbox.pop_front(), Some("second".to_string()));
    assert!(inbox.is_empty());
}

#[test]
fn test_pop_back() {
    let mut inbox = Inbox::new();
    inbox.push("first".to_string());
    inbox.push("second".to_string());

    assert_eq!(inbox.pop_back(), Some("second".to_string()));
    assert_eq!(inbox.len(), 1);
}

#[test]
fn test_clear() {
    let mut inbox = Inbox::new();
    inbox.push("a".to_string());
    inbox.push("b".to_string());
    inbox.clear();
    assert!(inbox.is_empty());
}

#[test]
fn test_iter() {
    let mut inbox = Inbox::new();
    inbox.push("a".to_string());
    inbox.push("b".to_string());

    let items: Vec<&String> = inbox.iter().collect();
    assert_eq!(items, vec!["a", "b"]);
}

#[test]
fn test_pop_empty() {
    let mut inbox = Inbox::new();
    assert!(inbox.pop_front().is_none());
    assert!(inbox.pop_back().is_none());
}

use loopal_session::message_log::{MessageFeed, MessageLogEntry};

#[test]
fn test_message_log_entry_creation() {
    let entry = MessageLogEntry::new("agent-a", "agent-b", "hello world");
    assert_eq!(entry.source, "agent-a");
    assert_eq!(entry.target, "agent-b");
    assert_eq!(entry.content_preview, "hello world");
}

#[test]
fn test_message_feed_record_and_iterate() {
    let mut feed = MessageFeed::new(10);
    feed.record(MessageLogEntry::new("a", "b", "msg1"));
    feed.record(MessageLogEntry::new("b", "a", "msg2"));

    assert_eq!(feed.len(), 2);
    assert!(!feed.is_empty());

    let entries: Vec<_> = feed.iter().collect();
    assert_eq!(entries[0].content_preview, "msg1");
    assert_eq!(entries[1].content_preview, "msg2");
}

#[test]
fn test_message_feed_evicts_oldest_at_capacity() {
    let mut feed = MessageFeed::new(3);
    feed.record(MessageLogEntry::new("a", "b", "1"));
    feed.record(MessageLogEntry::new("a", "b", "2"));
    feed.record(MessageLogEntry::new("a", "b", "3"));
    feed.record(MessageLogEntry::new("a", "b", "4"));

    assert_eq!(feed.len(), 3);
    let entries: Vec<_> = feed.iter().collect();
    assert_eq!(entries[0].content_preview, "2");
    assert_eq!(entries[2].content_preview, "4");
}

#[test]
fn test_message_feed_recent() {
    let mut feed = MessageFeed::new(10);
    for i in 0..5 {
        feed.record(MessageLogEntry::new("a", "b", format!("msg{i}")));
    }

    let recent: Vec<_> = feed.recent(2).collect();
    assert_eq!(recent.len(), 2);
    assert_eq!(recent[0].content_preview, "msg3");
    assert_eq!(recent[1].content_preview, "msg4");
}

#[test]
fn test_message_feed_empty() {
    let feed = MessageFeed::new(10);
    assert!(feed.is_empty());
    assert_eq!(feed.len(), 0);
    assert_eq!(feed.iter().count(), 0);
    assert_eq!(feed.recent(5).count(), 0);
}

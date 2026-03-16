use loopagent_context::compact_messages;
use loopagent_types::message::{Message, MessageRole};

#[test]
fn test_compact_keeps_system_and_last_n() {
    let mut msgs = vec![
        Message::system("sys"),
        Message::user("a"),
        Message::assistant("b"),
        Message::user("c"),
        Message::assistant("d"),
    ];
    compact_messages(&mut msgs, 2);
    assert_eq!(msgs.len(), 3); // system + last 2
    assert_eq!(msgs[0].role, MessageRole::System);
    assert_eq!(msgs[1].text_content(), "c");
    assert_eq!(msgs[2].text_content(), "d");
}

#[test]
fn test_compact_no_op_when_short() {
    let mut msgs = vec![Message::user("a"), Message::assistant("b")];
    compact_messages(&mut msgs, 5);
    assert_eq!(msgs.len(), 2);
}

#[test]
fn test_compact_no_system_messages() {
    let mut msgs = vec![
        Message::user("a"),
        Message::assistant("b"),
        Message::user("c"),
        Message::assistant("d"),
    ];
    compact_messages(&mut msgs, 2);
    assert_eq!(msgs.len(), 2);
    assert_eq!(msgs[0].text_content(), "c");
    assert_eq!(msgs[1].text_content(), "d");
}

#[test]
fn test_compact_exactly_at_limit() {
    // Messages exactly at keep_last + 1, should not compact
    // L5: messages.len() <= keep_last + 1 is true
    let mut msgs = vec![
        Message::user("a"),
        Message::assistant("b"),
    ];
    compact_messages(&mut msgs, 2);
    assert_eq!(msgs.len(), 2);
}

#[test]
fn test_compact_system_messages_with_few_non_system() {
    // L16: non_system_len <= keep_last is true
    let mut msgs = vec![
        Message::system("sys1"),
        Message::system("sys2"),
        Message::user("a"),
        Message::assistant("b"),
    ];
    compact_messages(&mut msgs, 3);
    // 4 total messages, keep_last=3, so len > keep_last + 1.
    // But non_system = 2, keep_last = 3, so non_system <= keep_last, no compaction.
    assert_eq!(msgs.len(), 4);
}

#[test]
fn test_compact_single_message() {
    let mut msgs = vec![Message::user("only")];
    compact_messages(&mut msgs, 5);
    assert_eq!(msgs.len(), 1);
}

#[test]
fn test_compact_keep_zero() {
    // Edge case: keep_last = 0
    let mut msgs = vec![
        Message::system("sys"),
        Message::user("a"),
        Message::assistant("b"),
    ];
    compact_messages(&mut msgs, 0);
    assert_eq!(msgs.len(), 1);
    assert_eq!(msgs[0].role, MessageRole::System);
}

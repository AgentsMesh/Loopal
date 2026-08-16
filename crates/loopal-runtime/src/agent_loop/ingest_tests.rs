use loopal_provider_api::Message;

use super::{extract_title, recent_context};

#[test]
fn title_is_trimmed_and_truncated_after_eighty_characters() {
    assert_eq!(extract_title("  first line  \nsecond"), "first line");

    let exact = "x".repeat(80);
    assert_eq!(extract_title(&exact), exact);
    assert_eq!(
        extract_title(&"x".repeat(81)),
        format!("{}…", "x".repeat(80))
    );
}

#[test]
fn recent_context_keeps_only_bounded_complete_messages() {
    assert!(recent_context(&[Message::user(&"x".repeat(8 * 1_024))]).is_empty());

    let messages = (0..17)
        .map(|index| Message::user(&format!("message-{index}")))
        .collect::<Vec<_>>();
    let context = recent_context(&messages);
    assert!(!context.lines().any(|line| line == "user: message-0"));
    assert!(context.lines().any(|line| line == "user: message-1"));
    assert!(context.lines().any(|line| line == "user: message-16"));
}

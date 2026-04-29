use loopal_protocol::{MessageSource, QualifiedAddress};
use loopal_session::types::{InboxOrigin, SessionMessage};
use loopal_tui::views::progress::message_to_lines;

fn flat(lines: &[ratatui::prelude::Line<'_>]) -> String {
    lines
        .iter()
        .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref()))
        .collect()
}

fn user_with_inbox(content: &str, source: MessageSource, summary: Option<&str>) -> SessionMessage {
    SessionMessage {
        role: "user".into(),
        content: content.into(),
        inbox: Some(InboxOrigin {
            message_id: "m-1".into(),
            source,
            summary: summary.map(str::to_string),
        }),
        ..Default::default()
    }
}

#[test]
fn test_human_inbox_origin_does_not_render_label() {
    let m = user_with_inbox("hi user typed", MessageSource::Human, None);
    let lines = message_to_lines(&m, 80);
    let text = flat(&lines);
    assert!(!text.contains("📨"));
    assert!(!text.contains("⏰"));
    assert!(text.contains("hi user typed"));
}

#[test]
fn test_agent_inbox_origin_renders_from_label() {
    let m = user_with_inbox(
        "ping",
        MessageSource::Agent(QualifiedAddress::local("worker")),
        None,
    );
    let lines = message_to_lines(&m, 80);
    let text = flat(&lines);
    assert!(text.contains("📨 from worker"));
    assert!(text.contains("ping"));
}

#[test]
fn test_scheduled_inbox_origin_renders_clock_label() {
    let m = user_with_inbox("tick", MessageSource::Scheduled, None);
    let text = flat(&message_to_lines(&m, 80));
    assert!(text.contains("⏰ scheduled"));
}

#[test]
fn test_channel_inbox_origin_renders_channel_label() {
    let m = user_with_inbox(
        "broadcast",
        MessageSource::Channel {
            channel: "general".into(),
            from: QualifiedAddress::local("bot"),
        },
        None,
    );
    let text = flat(&message_to_lines(&m, 80));
    assert!(text.contains("📡 #general/bot"));
}

#[test]
fn test_system_inbox_origin_renders_kind() {
    let m = user_with_inbox("hook", MessageSource::System("rewake".into()), None);
    let text = flat(&message_to_lines(&m, 80));
    assert!(text.contains("system:rewake"));
}

#[test]
fn test_inbox_summary_renders_alongside_content() {
    let m = user_with_inbox(
        "the long body",
        MessageSource::Agent(QualifiedAddress::local("a")),
        Some("ping"),
    );
    let text = flat(&message_to_lines(&m, 80));
    assert!(text.contains("ping"));
    assert!(text.contains("the long body"));
}

#[test]
fn test_qualified_remote_agent_address_renders_in_label() {
    let m = user_with_inbox(
        "from another hub",
        MessageSource::Agent(QualifiedAddress::remote(["hub-A"], "alpha")),
        None,
    );
    let text = flat(&message_to_lines(&m, 80));
    assert!(text.contains("📨 from hub-A/alpha"));
}

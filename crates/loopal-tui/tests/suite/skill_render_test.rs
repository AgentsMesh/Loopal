/// Skill invocation rendering tests: collapsed display, CJK truncation, args handling.
use loopal_protocol::SkillInvocation;
use loopal_session::types::SessionMessage;
use loopal_tui::views::progress::message_to_lines;

fn skill_msg(name: &str, args: &str) -> SessionMessage {
    SessionMessage {
        role: "user".to_string(),
        content: "expanded body ignored".to_string(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: Some(SkillInvocation {
            name: name.to_string(),
            user_args: args.to_string(),
        }),
    }
}

fn all_text(lines: &[ratatui::prelude::Line<'_>]) -> String {
    lines
        .iter()
        .map(|l| {
            l.spans
                .iter()
                .map(|s| s.content.as_ref())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Approximate display columns of a line via its span contents.
/// Heuristic: non-ASCII chars treated as 2 columns (matches CJK + block-drawing
/// characters used in the progress view). Not exact for all Unicode, but sufficient
/// for these tests where only ASCII, CJK, and ▎/▸ appear.
fn line_display_width(line: &ratatui::prelude::Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|s| {
            // Each char: CJK = 2 cols, ASCII = 1 col.
            s.content
                .chars()
                .map(|c| if c > '\x7f' { 2 } else { 1 })
                .sum::<usize>()
        })
        .sum()
}

#[test]
fn test_skill_collapsed_no_args() {
    let m = skill_msg("/commit", "");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("▸"), "should show arrow indicator");
    assert!(text.contains("/commit"), "should show skill name");
    assert!(
        !text.contains("expanded body"),
        "expanded body must be hidden"
    );
    // 1 content line + 1 empty separator = 2
    assert_eq!(
        lines.len(),
        2,
        "skill should collapse to single line + separator"
    );
}

#[test]
fn test_skill_collapsed_with_args() {
    let m = skill_msg("/github-pr", "fix the login bug");
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(text.contains("/github-pr"), "should show skill name");
    assert!(text.contains("fix the login bug"), "should show user args");
    assert!(
        !text.contains("expanded body"),
        "expanded body must be hidden"
    );
    assert_eq!(lines.len(), 2);
}

#[test]
fn test_skill_cjk_args_not_overflow() {
    // 20 CJK chars = 40 display columns. At width 50, with prefix+arrow+name overhead,
    // args should be truncated to fit without exceeding the line width.
    let cjk_args = "中文参数测试双宽字符截断验证用例超长文本";
    let m = skill_msg("/pr", cjk_args);
    let width: u16 = 50;
    let lines = message_to_lines(&m, width);

    let total_w = line_display_width(&lines[0]);
    assert!(
        total_w <= width as usize,
        "rendered line ({total_w} cols) must not exceed terminal width ({width} cols)"
    );
}

#[test]
fn test_skill_long_ascii_args_truncated() {
    let long_args = "a_b ".repeat(50); // 200 chars
    let m = skill_msg("/run", &long_args);
    let width: u16 = 60;
    let lines = message_to_lines(&m, width);

    let total_w = line_display_width(&lines[0]);
    assert!(
        total_w <= width as usize,
        "rendered line ({total_w} cols) must not exceed width ({width} cols)"
    );
}

#[test]
fn test_non_skill_user_message_unchanged() {
    let m = SessionMessage {
        role: "user".to_string(),
        content: "hello world".to_string(),
        tool_calls: Vec::new(),
        image_count: 0,
        skill_info: None,
    };
    let lines = message_to_lines(&m, 80);
    let text = all_text(&lines);
    assert!(
        text.contains("hello world"),
        "regular message should show content"
    );
    assert!(!text.contains("▸"), "regular message should not show arrow");
}

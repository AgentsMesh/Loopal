use loopal_protocol::{Question, QuestionOption};
use loopal_tui::views::{permission_inline, question_inline};
use loopal_view_state::{PendingPermission, PendingQuestion};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

fn opts(labels: &[&str]) -> Vec<QuestionOption> {
    labels
        .iter()
        .map(|l| QuestionOption {
            label: (*l).into(),
            description: String::new(),
        })
        .collect()
}

fn pq(opts: Vec<QuestionOption>, multi: bool) -> PendingQuestion {
    PendingQuestion::new(
        "id".into(),
        vec![Question {
            question: "Pick".into(),
            options: opts,
            allow_multiple: multi,
        }],
    )
}

fn cursor(q: &mut PendingQuestion, c: usize) {
    if let Some(s) = q.states.get_mut(q.current_question) {
        s.cursor = c;
    }
}

fn render_to_buffer(
    width: u16,
    height: u16,
    draw: impl FnOnce(&mut ratatui::Frame, Rect),
) -> String {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| {
        let area = Rect::new(0, 0, width, height);
        draw(f, area);
    })
    .unwrap();
    let buf = term.backend().buffer().clone();
    let mut out = String::new();
    for y in 0..buf.area.height {
        for x in 0..buf.area.width {
            out.push_str(buf[(x, y)].symbol());
        }
        out.push('\n');
    }
    out
}

fn render_capture_cursor(
    width: u16,
    height: u16,
    draw: impl FnOnce(&mut ratatui::Frame, Rect),
) -> Option<(u16, u16)> {
    let mut term = Terminal::new(TestBackend::new(width, height)).unwrap();
    term.draw(|f| {
        let area = Rect::new(0, 0, width, height);
        draw(f, area);
    })
    .unwrap();
    term.get_cursor_position().ok().map(|p| (p.x, p.y))
}

#[test]
fn question_renders_question_text_and_options() {
    let q = pq(opts(&["A", "B"]), false);
    let s = render_to_buffer(60, 6, |f, area| question_inline::render(f, &q, area, None));
    assert!(s.contains("? Pick"));
    assert!(s.contains("A"));
    assert!(s.contains("B"));
    assert!(s.contains("Other"));
}

#[test]
fn question_single_select_marks_cursor_with_filled_radio() {
    let mut q = pq(opts(&["A", "B", "C"]), false);
    cursor(&mut q, 1);
    let s = render_to_buffer(60, 7, |f, area| question_inline::render(f, &q, area, None));
    let lines: Vec<&str> = s.lines().collect();
    assert!(lines.iter().any(|l| l.contains("(•)") && l.contains("B")));
    assert!(
        !lines
            .iter()
            .any(|l| l.contains("(•)") && (l.contains("A") || l.contains("C")))
    );
}

#[test]
fn question_multi_select_uses_checkbox_and_other_box() {
    let mut q = pq(opts(&["A", "B"]), true);
    cursor(&mut q, 0);
    q.toggle();
    cursor(&mut q, 2);
    q.toggle();
    let s = render_to_buffer(60, 7, |f, area| question_inline::render(f, &q, area, None));
    let lines: Vec<&str> = s.lines().collect();
    assert!(lines.iter().any(|l| l.contains("[x]") && l.contains("A")));
    assert!(lines.iter().any(|l| l.contains("[ ]") && l.contains("B")));
    assert!(
        lines
            .iter()
            .any(|l| l.contains("[x]") && l.contains("Other"))
    );
}

#[test]
fn question_renders_free_text_input_when_cursor_on_other() {
    let mut q = pq(opts(&["A"]), false);
    cursor(&mut q, 1);
    "hello".chars().for_each(|c| q.free_text_insert_char(c));
    let s = render_to_buffer(60, 7, |f, area| question_inline::render(f, &q, area, None));
    assert!(s.contains("> hello"), "buffer was:\n{s}");
}

#[test]
fn question_renders_chinese_free_text() {
    let mut q = pq(opts(&[]), false);
    "你好".chars().for_each(|c| q.free_text_insert_char(c));
    let s = render_to_buffer(60, 6, |f, area| question_inline::render(f, &q, area, None));
    assert!(s.contains("你") && s.contains("好"), "buffer was:\n{s}");
}

#[test]
fn question_long_text_wraps_into_multiple_lines() {
    let q = PendingQuestion::new(
        "id".into(),
        vec![Question {
            question: "very ".repeat(30) + "long question",
            options: opts(&["A"]),
            allow_multiple: false,
        }],
    );
    let h = question_inline::height(&q, 30);
    assert!(h > 4, "long question should wrap, got h={h}");
}

#[test]
fn question_cursor_position_accounts_for_wide_chars() {
    let mut q = pq(opts(&[]), false);
    "你好".chars().for_each(|c| q.free_text_insert_char(c));
    let pos = render_capture_cursor(60, 6, |f, area| question_inline::render(f, &q, area, None));
    let pos = pos.expect("cursor must be set when on Other");
    let prefix_w: u16 = 6;
    let typed_w: u16 = 4;
    assert_eq!(pos.0, prefix_w + typed_w, "cursor col mismatch: {pos:?}");
}

#[test]
fn question_cursor_position_with_ascii() {
    let mut q = pq(opts(&[]), false);
    "abc".chars().for_each(|c| q.free_text_insert_char(c));
    let pos = render_capture_cursor(60, 6, |f, area| question_inline::render(f, &q, area, None));
    let pos = pos.expect("cursor must be set when on Other");
    let prefix_w: u16 = 6;
    let typed_w: u16 = 3;
    assert_eq!(pos.0, prefix_w + typed_w);
}

#[test]
fn question_height_capped_keeps_render_safe() {
    let opts_vec = opts(&["a", "b", "c", "d", "e", "f", "g", "h"]);
    let q = pq(opts_vec, false);
    let s = render_to_buffer(60, 4, |f, area| question_inline::render(f, &q, area, None));
    assert!(s.contains("? Pick"));
}

#[test]
fn permission_renders_tool_name_and_keys() {
    let p = PendingPermission {
        id: "1".into(),
        name: "Bash".into(),
        input: serde_json::json!({"cmd": "ls"}),
    };
    let s = render_to_buffer(60, 6, |f, area| {
        permission_inline::render_prepared(f, &permission_inline::prepare(&p), area, None)
    });
    assert!(s.contains("⚠ Tool: Bash"));
    assert!(s.contains("[y] Allow"));
    assert!(s.contains("[n] Deny"));
    assert!(s.contains("Esc Cancel"));
}

#[test]
fn permission_truncates_large_input_with_ellipsis() {
    let mut big = serde_json::Map::new();
    for i in 0..20 {
        big.insert(format!("k{i}"), serde_json::json!(i));
    }
    let p = PendingPermission {
        id: "1".into(),
        name: "X".into(),
        input: serde_json::Value::Object(big),
    };
    let s = render_to_buffer(80, 12, |f, area| {
        permission_inline::render_prepared(f, &permission_inline::prepare(&p), area, None)
    });
    assert!(s.contains("more lines"), "truncation marker missing:\n{s}");
}

#[test]
fn permission_height_for_simple_input() {
    let p = PendingPermission {
        id: "1".into(),
        name: "X".into(),
        input: serde_json::json!({}),
    };
    assert_eq!(permission_inline::height(&p, 80), 3);
}

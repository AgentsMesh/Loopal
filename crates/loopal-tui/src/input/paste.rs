use std::collections::HashMap;

use crate::app::App;
use crate::event::{AppEvent, EventHandler};
use crate::input::paste_clipboard::read_clipboard;
use loopal_protocol::ImageAttachment;

#[derive(Debug)]
pub enum PasteResult {
    Image(ImageAttachment),
    Text(String),
    Empty,
    Unavailable,
}

pub fn spawn_paste(events: &EventHandler) {
    let tx = events.sender();
    tokio::task::spawn_blocking(move || {
        let _ = tx.blocking_send(AppEvent::Paste(read_clipboard()));
    });
}

const LARGE_PASTE_LINE_THRESHOLD: usize = 5;
const LARGE_PASTE_CHAR_THRESHOLD: usize = 500;

pub fn apply_paste_result(app: &mut App, result: PasteResult) {
    let in_modal = app.with_active_conversation(|conv| {
        conv.pending_question.is_some() || conv.pending_permission.is_some()
    });
    match result {
        PasteResult::Image(attachment) => {
            if in_modal {
                app.set_transient_status("Image paste ignored — close the dialog first.");
                return;
            }
            app.attach_image(attachment);
        }
        PasteResult::Text(text) => {
            if in_modal && crate::question_ops::route_paste(app, &text) {
                return;
            }
            if in_modal {
                app.set_transient_status("Paste ignored — focus the Other-input row.");
                return;
            }
            let line_count = text.lines().count().max(1);
            if line_count > LARGE_PASTE_LINE_THRESHOLD || text.len() > LARGE_PASTE_CHAR_THRESHOLD {
                let placeholder = generate_placeholder(&text, line_count, &app.paste_map);
                app.paste_map.insert(placeholder.clone(), text);
                app.input.insert_str(app.input_cursor, &placeholder);
                app.input_cursor += placeholder.len();
            } else {
                app.input.insert_str(app.input_cursor, &text);
                app.input_cursor += text.len();
            }
        }
        PasteResult::Empty => {}
        PasteResult::Unavailable => {
            app.set_transient_status("Clipboard not available (SSH/headless session?)");
        }
    }
}

/// Expand all paste placeholders back to their full content.
pub fn expand_paste_placeholders(text: &str, paste_map: &HashMap<String, String>) -> String {
    if paste_map.is_empty() {
        return text.to_string();
    }
    let mut result = text.to_string();
    for (placeholder, content) in paste_map {
        result = result.replace(placeholder, content);
    }
    result
}

/// Check if a substring looks like a paste placeholder.
pub fn is_paste_placeholder(s: &str) -> bool {
    s.starts_with("[paste:") && s.ends_with(']') && s.len() > 8
}

fn generate_placeholder(
    content: &str,
    _line_count: usize,
    existing: &HashMap<String, String>,
) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    let base = format!("[paste:{:016x}]", hasher.finish());
    if !existing.contains_key(&base) {
        return base;
    }
    let mut suffix = 2u64;
    loop {
        suffix.hash(&mut hasher);
        let candidate = format!("[paste:{:016x}]", hasher.finish());
        if !existing.contains_key(&candidate) {
            return candidate;
        }
        suffix += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_paste_no_placeholder() {
        let text = "hello\nworld";
        assert!(text.lines().count() <= LARGE_PASTE_LINE_THRESHOLD);
        assert!(text.len() <= LARGE_PASTE_CHAR_THRESHOLD);
    }

    #[test]
    fn large_paste_generates_hash_placeholder() {
        let text = "line1\nline2\nline3\nline4\nline5\nline6";
        let map = HashMap::new();
        let placeholder = generate_placeholder(text, 6, &map);
        assert!(placeholder.starts_with("[paste:"), "got: {placeholder}");
        assert!(placeholder.ends_with(']'));
        assert!(is_paste_placeholder(&placeholder));
    }

    #[test]
    fn placeholder_dedup() {
        let text = "some content";
        let first = generate_placeholder(text, 1, &HashMap::new());
        let map = HashMap::from([(first.clone(), text.into())]);
        let second = generate_placeholder(text, 1, &map);
        assert_ne!(
            first, second,
            "duplicates should get different placeholders"
        );
        assert!(is_paste_placeholder(&second));
    }

    #[test]
    fn expand_placeholders() {
        let text = "line1\nline2\nline3\nline4\nline5\nline6";
        let placeholder = generate_placeholder(text, 6, &HashMap::new());
        let map = HashMap::from([(placeholder.clone(), "full\ncontent".into())]);
        let input = format!("before {placeholder} after");
        let expanded = expand_paste_placeholders(&input, &map);
        assert_eq!(expanded, "before full\ncontent after");
    }

    #[test]
    fn user_text_not_mistaken_for_placeholder() {
        // Regression: human-readable placeholder format was guessable
        assert!(!is_paste_placeholder("[Pasted Text: 6 lines]"));
        assert!(!is_paste_placeholder("hello"));
    }
}

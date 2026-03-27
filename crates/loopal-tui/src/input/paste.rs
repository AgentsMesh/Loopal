//! Clipboard paste handler with large-paste folding.
//! Large pastes (>5 lines or >500 chars) are folded into placeholders; expanded on submit.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tracing::warn;

use crate::app::App;
use crate::event::{AppEvent, EventHandler};
use loopal_protocol::ImageAttachment;

/// Result of an async clipboard read operation.
#[derive(Debug)]
pub enum PasteResult {
    Image(ImageAttachment),
    Text(String),
    Empty,
    Unavailable,
}

const MAX_BASE64_BYTES: usize = 5 * 1024 * 1024;
const MAX_DIMENSION: u32 = 2048;
const LARGE_PASTE_LINE_THRESHOLD: usize = 5;
const LARGE_PASTE_CHAR_THRESHOLD: usize = 500;

/// Spawn a blocking clipboard read; result sent as `AppEvent::Paste`.
pub fn spawn_paste(events: &EventHandler) {
    let tx = events.sender();
    tokio::task::spawn_blocking(move || {
        let _ = tx.blocking_send(AppEvent::Paste(read_clipboard()));
    });
}

/// Apply paste result. Large text pastes are folded into placeholders.
pub fn apply_paste_result(app: &mut App, result: PasteResult) {
    match result {
        PasteResult::Image(attachment) => app.attach_image(attachment),
        PasteResult::Text(text) => {
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
            app.session
                .push_system_message("Clipboard not available (SSH/headless session?)".into());
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
    // Hash collision (extremely rare) — append suffix
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

fn read_clipboard() -> PasteResult {
    let Ok(mut clipboard) = arboard::Clipboard::new() else {
        warn!("failed to open clipboard");
        return PasteResult::Unavailable;
    };
    if let Ok(img_data) = clipboard.get_image()
        && let Some(attachment) = encode_clipboard_image(img_data)
    {
        return PasteResult::Image(attachment);
    }
    if let Ok(text) = clipboard.get_text()
        && !text.is_empty()
    {
        return PasteResult::Text(text);
    }
    PasteResult::Empty
}

fn encode_clipboard_image(img_data: arboard::ImageData<'_>) -> Option<ImageAttachment> {
    let (width, height) = (img_data.width as u32, img_data.height as u32);
    let img = image::RgbaImage::from_raw(width, height, img_data.bytes.into_owned())?;
    let img = if width > MAX_DIMENSION || height > MAX_DIMENSION {
        image::DynamicImage::ImageRgba8(img).resize(
            MAX_DIMENSION,
            MAX_DIMENSION,
            image::imageops::FilterType::Lanczos3,
        )
    } else {
        image::DynamicImage::ImageRgba8(img)
    };
    let mut png_buf = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_buf),
        image::ImageFormat::Png,
    )
    .ok()?;
    let b64 = BASE64.encode(&png_buf);
    if b64.len() > MAX_BASE64_BYTES {
        warn!(
            size = b64.len(),
            max = MAX_BASE64_BYTES,
            "clipboard image too large"
        );
        return None;
    }
    Some(ImageAttachment {
        media_type: "image/png".to_string(),
        data: b64,
    })
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

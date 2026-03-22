//! Clipboard paste handler with large-paste folding.
//!
//! Large pastes (>5 lines or >500 chars) are folded into `[Pasted Text: N lines]`
//! placeholders. Full content is stored in `App.paste_map` and expanded on submit.

use std::collections::HashMap;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tracing::warn;

use loopal_protocol::ImageAttachment;
use crate::app::App;
use crate::event::{AppEvent, EventHandler};

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
            if line_count > LARGE_PASTE_LINE_THRESHOLD
                || text.len() > LARGE_PASTE_CHAR_THRESHOLD
            {
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
            app.session.push_system_message(
                "Clipboard not available (SSH/headless session?)".into(),
            );
        }
    }
}

/// Expand all paste placeholders back to their full content.
pub fn expand_paste_placeholders(text: &str, paste_map: &HashMap<String, String>) -> String {
    if paste_map.is_empty() { return text.to_string(); }
    let mut result = text.to_string();
    for (placeholder, content) in paste_map {
        result = result.replace(placeholder, content);
    }
    result
}

/// Check if a substring looks like a paste placeholder.
pub fn is_paste_placeholder(s: &str) -> bool {
    s.starts_with("[Pasted Text: ") && s.ends_with(']')
}

fn generate_placeholder(
    content: &str, line_count: usize, existing: &HashMap<String, String>,
) -> String {
    let metric = if line_count > LARGE_PASTE_LINE_THRESHOLD {
        format!("{} lines", line_count)
    } else {
        format!("{} chars", content.len())
    };
    let base = format!("[Pasted Text: {}]", metric);
    if !existing.contains_key(&base) { return base; }
    let mut suffix = 2;
    loop {
        let candidate = format!("[Pasted Text: {} #{}]", metric, suffix);
        if !existing.contains_key(&candidate) { return candidate; }
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
        image::DynamicImage::ImageRgba8(img)
            .resize(MAX_DIMENSION, MAX_DIMENSION, image::imageops::FilterType::Lanczos3)
    } else {
        image::DynamicImage::ImageRgba8(img)
    };
    let mut png_buf = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut png_buf), image::ImageFormat::Png).ok()?;
    let b64 = BASE64.encode(&png_buf);
    if b64.len() > MAX_BASE64_BYTES {
        warn!(size = b64.len(), max = MAX_BASE64_BYTES, "clipboard image too large");
        return None;
    }
    Some(ImageAttachment { media_type: "image/png".to_string(), data: b64 })
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
    fn large_paste_generates_placeholder() {
        let text = "line1\nline2\nline3\nline4\nline5\nline6";
        let map = HashMap::new();
        assert_eq!(generate_placeholder(text, 6, &map), "[Pasted Text: 6 lines]");
    }

    #[test]
    fn placeholder_dedup() {
        let map = HashMap::from([("[Pasted Text: 6 lines]".into(), "x".into())]);
        assert_eq!(generate_placeholder("x", 6, &map), "[Pasted Text: 6 lines #2]");
    }

    #[test]
    fn expand_placeholders() {
        let map = HashMap::from([
            ("[Pasted Text: 6 lines]".into(), "full\ncontent".into()),
        ]);
        let expanded = expand_paste_placeholders("before [Pasted Text: 6 lines] after", &map);
        assert_eq!(expanded, "before full\ncontent after");
    }

    #[test]
    fn char_based_placeholder() {
        let text = "a".repeat(501);
        assert_eq!(generate_placeholder(&text, 1, &HashMap::new()), "[Pasted Text: 501 chars]");
    }
}

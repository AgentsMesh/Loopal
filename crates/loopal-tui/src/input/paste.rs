//! Clipboard paste handler: reads image or text from the system clipboard.
//!
//! Clipboard I/O runs on a blocking thread via `spawn_blocking` to avoid
//! stalling the TUI event loop. Results are sent back as `PasteResult`.

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tracing::warn;

use loopal_protocol::ImageAttachment;

use crate::app::App;
use crate::event::{AppEvent, EventHandler};

/// Result of an async clipboard read operation.
#[derive(Debug)]
pub enum PasteResult {
    /// Clipboard contained an image, now encoded as base64 PNG.
    Image(ImageAttachment),
    /// Clipboard contained text.
    Text(String),
    /// Clipboard was empty or unreadable.
    Empty,
    /// Clipboard access failed (no display server, SSH, etc.).
    Unavailable,
}

/// Maximum base64 size (~5 MB). Images exceeding this are rejected.
const MAX_BASE64_BYTES: usize = 5 * 1024 * 1024;
/// Maximum pixel dimension (width or height) for resizing.
const MAX_DIMENSION: u32 = 2048;

/// Spawn a blocking clipboard read on a background thread.
/// Result is sent back as `AppEvent::Paste` via the event channel.
pub fn spawn_paste(events: &EventHandler) {
    let tx = events.sender();
    tokio::task::spawn_blocking(move || {
        let result = read_clipboard();
        let _ = tx.blocking_send(AppEvent::Paste(result));
    });
}

/// Apply the result of an async clipboard read to the app state.
pub fn apply_paste_result(app: &mut App, result: PasteResult) {
    match result {
        PasteResult::Image(attachment) => {
            app.attach_image(attachment);
        }
        PasteResult::Text(text) => {
            app.input.insert_str(app.input_cursor, &text);
            app.input_cursor += text.len();
        }
        PasteResult::Empty => {}
        PasteResult::Unavailable => {
            app.session.push_system_message(
                "Clipboard not available (SSH/headless session?)".into(),
            );
        }
    }
}

/// Read the clipboard synchronously. Returns a `PasteResult`.
///
/// `Send + 'static` safe — can be passed to `tokio::task::spawn_blocking`.
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

/// Encode clipboard image data as a base64 PNG `ImageAttachment`.
fn encode_clipboard_image(img_data: arboard::ImageData<'_>) -> Option<ImageAttachment> {
    let width = img_data.width as u32;
    let height = img_data.height as u32;
    let rgba = img_data.bytes.into_owned();

    let img = image::RgbaImage::from_raw(width, height, rgba)?;

    let img = if width > MAX_DIMENSION || height > MAX_DIMENSION {
        image::DynamicImage::ImageRgba8(img)
            .resize(MAX_DIMENSION, MAX_DIMENSION, image::imageops::FilterType::Lanczos3)
    } else {
        image::DynamicImage::ImageRgba8(img)
    };

    let mut png_buf = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_buf);
    img.write_to(&mut cursor, image::ImageFormat::Png).ok()?;

    let b64 = BASE64.encode(&png_buf);

    if b64.len() > MAX_BASE64_BYTES {
        warn!(size = b64.len(), max = MAX_BASE64_BYTES, "clipboard image too large, skipping");
        return None;
    }

    Some(ImageAttachment {
        media_type: "image/png".to_string(),
        data: b64,
    })
}

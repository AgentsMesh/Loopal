//! Rich user input: text + optional attached images.
//!
//! Replaces `String` in the TUI → Runtime pipeline. Agent-to-agent
//! messaging remains text-only thanks to `From<String>` / `From<&str>`.

use serde::{Deserialize, Serialize};

/// User message content carrying text and optional image attachments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserContent {
    pub text: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageAttachment>,
}

/// A single image attachment encoded as base64.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImageAttachment {
    /// MIME type, e.g. `"image/png"`, `"image/jpeg"`.
    pub media_type: String,
    /// Base64-encoded image data.
    pub data: String,
}

impl UserContent {
    /// Create text-only content (no images).
    pub fn text_only(text: impl Into<String>) -> Self {
        Self { text: text.into(), images: Vec::new() }
    }

    /// Whether this content contains any images.
    pub fn has_images(&self) -> bool {
        !self.images.is_empty()
    }

    /// Short preview of the text content (max ~80 chars, safe for multi-byte).
    pub fn text_preview(&self) -> &str {
        let s = self.text.as_str();
        if s.len() <= 80 {
            s
        } else {
            let mut end = 80;
            while end > 0 && !s.is_char_boundary(end) {
                end -= 1;
            }
            &s[..end]
        }
    }
}

impl From<String> for UserContent {
    fn from(text: String) -> Self {
        Self { text, images: Vec::new() }
    }
}

impl From<&str> for UserContent {
    fn from(text: &str) -> Self {
        Self { text: text.to_string(), images: Vec::new() }
    }
}

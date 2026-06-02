use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageMime {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageMime {
    pub fn from_magic(bytes: &[u8]) -> Option<Self> {
        if bytes.len() >= 8 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" {
            return Some(Self::Png);
        }
        if bytes.len() >= 3 && &bytes[..3] == b"\xff\xd8\xff" {
            return Some(Self::Jpeg);
        }
        if bytes.len() >= 6 && (&bytes[..6] == b"GIF87a" || &bytes[..6] == b"GIF89a") {
            return Some(Self::Gif);
        }
        if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
            return Some(Self::Webp);
        }
        None
    }

    pub fn from_mime_str(mime: &str) -> Option<Self> {
        match mime {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::Webp),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolImageBlock {
    Inline {
        media_type: String,
        data: String,
    },
    SessionResource {
        id: String,
        media_type: String,
        byte_size: usize,
    },
}

impl ToolImageBlock {
    pub fn media_type(&self) -> &str {
        match self {
            Self::Inline { media_type, .. } | Self::SessionResource { media_type, .. } => {
                media_type
            }
        }
    }

    pub fn byte_size(&self) -> usize {
        match self {
            Self::Inline { data, .. } => data.len() * 3 / 4,
            Self::SessionResource { byte_size, .. } => *byte_size,
        }
    }

    // Content identity: Inline → base64 data, SessionResource → content-addressed
    // id. Distinguishes different images at the same path (byte_size alone would
    // collide two same-length-but-different images).
    pub fn content_key(&self) -> &str {
        match self {
            Self::Inline { data, .. } => data,
            Self::SessionResource { id, .. } => id,
        }
    }

    pub fn is_inline(&self) -> bool {
        matches!(self, Self::Inline { .. })
    }

    pub fn as_inline(&self) -> Option<(&str, &str)> {
        match self {
            Self::Inline { media_type, data } => Some((media_type.as_str(), data.as_str())),
            Self::SessionResource { .. } => None,
        }
    }

    pub fn inline(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        Self::Inline {
            media_type: media_type.into(),
            data: data.into(),
        }
    }

    pub fn session_resource(
        id: impl Into<String>,
        media_type: impl Into<String>,
        byte_size: usize,
    ) -> Self {
        Self::SessionResource {
            id: id.into(),
            media_type: media_type.into(),
            byte_size,
        }
    }
}

use std::io::Read;

use base64::engine::general_purpose::STANDARD;
use base64::read::DecoderReader;
use loopal_tool_invocation::ImageMime;
use thiserror::Error;

#[derive(PartialEq, Eq)]
pub struct ValidatedInlineImage {
    mime: ImageMime,
    bytes: Vec<u8>,
}

impl ValidatedInlineImage {
    pub fn mime(&self) -> ImageMime {
        self.mime
    }

    pub fn media_type(&self) -> &'static str {
        self.mime.as_str()
    }

    pub fn byte_size(&self) -> usize {
        self.bytes.len()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

impl std::fmt::Debug for ValidatedInlineImage {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ValidatedInlineImage")
            .field("mime", &self.mime)
            .field("byte_size", &self.bytes.len())
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InlineImageError {
    #[error("inline image base64 is malformed")]
    MalformedBase64,
    #[error("inline image is empty")]
    Empty,
    #[error("inline image is {actual_bytes} decoded bytes; limit is {max_bytes} bytes")]
    DecodedByteLimitExceeded {
        actual_bytes: usize,
        max_bytes: usize,
    },
    #[error("declared image MIME is unsupported")]
    UnsupportedDeclaredMime,
    #[error("decoded image format is unsupported or malformed")]
    UnknownImageFormat,
    #[error("declared image MIME does not match decoded image format")]
    MimeMismatch,
}

pub fn validate_inline_image(
    declared_mime: &str,
    encoded: &str,
    max_decoded_bytes: usize,
) -> Result<ValidatedInlineImage, InlineImageError> {
    let declared =
        ImageMime::from_mime_str(declared_mime).ok_or(InlineImageError::UnsupportedDeclaredMime)?;
    let bytes = decode_bounded(encoded, max_decoded_bytes)?;
    validate_decoded(declared, bytes, max_decoded_bytes)
}

pub fn validate_decoded_image(
    declared_mime: &str,
    bytes: Vec<u8>,
    max_decoded_bytes: usize,
) -> Result<ValidatedInlineImage, InlineImageError> {
    let declared =
        ImageMime::from_mime_str(declared_mime).ok_or(InlineImageError::UnsupportedDeclaredMime)?;
    validate_decoded(declared, bytes, max_decoded_bytes)
}

fn validate_decoded(
    declared: ImageMime,
    bytes: Vec<u8>,
    max_decoded_bytes: usize,
) -> Result<ValidatedInlineImage, InlineImageError> {
    if bytes.is_empty() {
        return Err(InlineImageError::Empty);
    }
    if bytes.len() > max_decoded_bytes {
        return Err(InlineImageError::DecodedByteLimitExceeded {
            actual_bytes: bytes.len(),
            max_bytes: max_decoded_bytes,
        });
    }
    let detected = ImageMime::from_magic(&bytes).ok_or(InlineImageError::UnknownImageFormat)?;
    if declared != detected {
        return Err(InlineImageError::MimeMismatch);
    }
    Ok(ValidatedInlineImage {
        mime: detected,
        bytes,
    })
}

fn decode_bounded(encoded: &str, max_bytes: usize) -> Result<Vec<u8>, InlineImageError> {
    let decoded_size = canonical_decoded_size(encoded)?;
    if decoded_size > max_bytes {
        return Err(InlineImageError::DecodedByteLimitExceeded {
            actual_bytes: decoded_size,
            max_bytes,
        });
    }
    let mut bytes = Vec::with_capacity(decoded_size);
    DecoderReader::new(encoded.as_bytes(), &STANDARD)
        .read_to_end(&mut bytes)
        .map_err(|_| InlineImageError::MalformedBase64)?;
    if bytes.is_empty() {
        return Err(InlineImageError::Empty);
    }
    Ok(bytes)
}

fn canonical_decoded_size(encoded: &str) -> Result<usize, InlineImageError> {
    if encoded.is_empty() {
        return Ok(0);
    }
    if !encoded.len().is_multiple_of(4) {
        return Err(InlineImageError::MalformedBase64);
    }
    let padding = encoded
        .as_bytes()
        .iter()
        .rev()
        .take_while(|byte| **byte == b'=')
        .count();
    if padding > 2 {
        return Err(InlineImageError::MalformedBase64);
    }
    Ok(encoded.len() / 4 * 3 - padding)
}

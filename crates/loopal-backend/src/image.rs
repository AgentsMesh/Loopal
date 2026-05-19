use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::ImageResult;
use loopal_tool_invocation::ImageMime;

use crate::limits::ResourceLimits;
use crate::path;

fn parse_dimensions(bytes: &[u8], mime: ImageMime) -> Option<(u32, u32)> {
    match mime {
        ImageMime::Png => parse_png(bytes),
        ImageMime::Jpeg => parse_jpeg(bytes),
        ImageMime::Gif => parse_gif(bytes),
        ImageMime::Webp => parse_webp(bytes),
    }
}

fn parse_png(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 24 {
        return None;
    }
    let w = u32::from_be_bytes(bytes[16..20].try_into().ok()?);
    let h = u32::from_be_bytes(bytes[20..24].try_into().ok()?);
    Some((w, h))
}

fn parse_gif(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 10 {
        return None;
    }
    let w = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
    let h = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
    Some((w, h))
}

fn parse_jpeg(bytes: &[u8]) -> Option<(u32, u32)> {
    let mut i = 2;
    while i + 8 < bytes.len() {
        if bytes[i] != 0xFF {
            return None;
        }
        let marker = bytes[i + 1];
        if marker == 0xFF {
            i += 1;
            continue;
        }
        let is_sof =
            (0xC0..=0xCF).contains(&marker) && marker != 0xC4 && marker != 0xC8 && marker != 0xCC;
        if is_sof {
            let h = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
            let w = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
            return Some((w, h));
        }
        let seg_len = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
        i += 2 + seg_len;
    }
    None
}

fn parse_webp(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() < 16 {
        return None;
    }
    match &bytes[12..16] {
        b"VP8 " => {
            if bytes.len() < 30 {
                return None;
            }
            let w = u16::from_le_bytes([bytes[26], bytes[27]]) as u32 & 0x3fff;
            let h = u16::from_le_bytes([bytes[28], bytes[29]]) as u32 & 0x3fff;
            Some((w, h))
        }
        b"VP8L" => {
            if bytes.len() < 25 {
                return None;
            }
            if bytes[20] != 0x2F {
                return None;
            }
            let b0 = bytes[21] as u32;
            let b1 = bytes[22] as u32;
            let b2 = bytes[23] as u32;
            let b3 = bytes[24] as u32;
            let w = (b0 | ((b1 & 0x3f) << 8)) + 1;
            let h = ((b1 >> 6) | (b2 << 2) | ((b3 & 0x0f) << 10)) + 1;
            Some((w, h))
        }
        b"VP8X" => {
            if bytes.len() < 30 {
                return None;
            }
            let w = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]) + 1;
            let h = u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]) + 1;
            Some((w, h))
        }
        _ => None,
    }
}

pub async fn read_image(
    cwd: &std::path::Path,
    raw_path: &str,
    policy: Option<&loopal_config::ResolvedPolicy>,
    limits: &ResourceLimits,
) -> Result<ImageResult, ToolIoError> {
    let resolved = path::resolve(cwd, raw_path, false, policy)?;
    let bytes = tokio::fs::read(&resolved).await?;
    let size = bytes.len() as u64;
    if size > limits.image_max_bytes {
        return Err(ToolIoError::TooLarge {
            path: raw_path.to_string(),
            size,
            limit: limits.image_max_bytes,
        });
    }
    let mime = ImageMime::from_magic(&bytes).ok_or_else(|| {
        ToolIoError::Other(format!(
            "unsupported image format (expected PNG/JPEG/GIF/WEBP): {raw_path}"
        ))
    })?;
    let (w, h) = parse_dimensions(&bytes, mime)
        .ok_or_else(|| ToolIoError::Other(format!("malformed image dimensions: {raw_path}")))?;
    let pixels = w as u64 * h as u64;
    if pixels > limits.image_max_pixels {
        return Err(ToolIoError::Other(format!(
            "image too many pixels: {w}x{h} ({pixels} > {})",
            limits.image_max_pixels
        )));
    }
    let data = STANDARD.encode(&bytes);
    Ok(ImageResult {
        media_type: mime.as_str().to_string(),
        data,
        dimensions: (w, h),
        byte_size: bytes.len(),
    })
}

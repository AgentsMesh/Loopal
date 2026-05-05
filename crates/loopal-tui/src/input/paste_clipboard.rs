use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use tracing::warn;

use loopal_protocol::ImageAttachment;

use super::paste::PasteResult;

const MAX_BASE64_BYTES: usize = 5 * 1024 * 1024;
const MAX_DIMENSION: u32 = 2048;

pub(super) fn read_clipboard() -> PasteResult {
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

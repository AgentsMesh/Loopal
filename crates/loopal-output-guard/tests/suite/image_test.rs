use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_output_guard::{InlineImageError, validate_decoded_image, validate_inline_image};
use loopal_tool_invocation::ImageMime;

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nbody";
const JPEG: &[u8] = b"\xff\xd8\xffbody";
const GIF: &[u8] = b"GIF89abody";
const WEBP: &[u8] = b"RIFF\x04\0\0\0WEBPbody";

#[test]
fn accepts_each_supported_image_mime() {
    for (mime, bytes, expected) in [
        ("image/png", PNG, ImageMime::Png),
        ("image/jpeg", JPEG, ImageMime::Jpeg),
        ("image/gif", GIF, ImageMime::Gif),
        ("image/webp", WEBP, ImageMime::Webp),
    ] {
        let encoded = STANDARD.encode(bytes);
        let image = validate_inline_image(mime, &encoded, bytes.len()).unwrap();
        assert_eq!(image.mime(), expected);
        assert_eq!(image.media_type(), mime);
        assert_eq!(image.byte_size(), bytes.len());
        assert_eq!(image.bytes(), bytes);
    }
}

#[test]
fn validated_image_owns_bytes_without_debugging_content() {
    let encoded = STANDARD.encode(PNG);
    let image = validate_inline_image("image/png", &encoded, PNG.len()).unwrap();
    let debug = format!("{image:?}");
    assert!(debug.contains("byte_size"));
    assert!(!debug.contains(&encoded));
    assert_eq!(image.into_bytes(), PNG);
}

#[test]
fn validates_owned_decoded_image_bytes() {
    let image = validate_decoded_image("image/png", PNG.to_vec(), PNG.len()).unwrap();
    assert_eq!(image.media_type(), "image/png");
    assert_eq!(image.into_bytes(), PNG);
}

#[test]
fn decoded_validation_rejects_empty_oversized_unknown_and_mismatch() {
    assert_eq!(
        validate_decoded_image("image/png", Vec::new(), 10),
        Err(InlineImageError::Empty)
    );
    assert_eq!(
        validate_decoded_image("image/png", PNG.to_vec(), PNG.len() - 1),
        Err(InlineImageError::DecodedByteLimitExceeded {
            actual_bytes: PNG.len(),
            max_bytes: PNG.len() - 1,
        })
    );
    assert_eq!(
        validate_decoded_image("image/png", b"unknown".to_vec(), 10),
        Err(InlineImageError::UnknownImageFormat)
    );
    assert_eq!(
        validate_decoded_image("image/jpeg", PNG.to_vec(), PNG.len()),
        Err(InlineImageError::MimeMismatch)
    );
}

#[test]
fn rejects_malformed_noncanonical_and_empty_base64() {
    assert_eq!(
        validate_inline_image("image/png", "%%%%", 10),
        Err(InlineImageError::MalformedBase64)
    );
    assert_eq!(
        validate_inline_image("image/png", "%%%", 10),
        Err(InlineImageError::MalformedBase64)
    );
    assert_eq!(
        validate_inline_image("image/png", "iVBORw0KGgo", 10),
        Err(InlineImageError::MalformedBase64)
    );
    assert_eq!(
        validate_inline_image("image/png", "Zh==", 10),
        Err(InlineImageError::MalformedBase64)
    );
    assert_eq!(
        validate_inline_image("image/png", "", 10),
        Err(InlineImageError::Empty)
    );
    assert_eq!(
        validate_inline_image("image/png", "====", 10),
        Err(InlineImageError::MalformedBase64)
    );
}

#[test]
fn rejects_canonical_empty_payload() {
    let error = validate_inline_image("image/png", "", 0).unwrap_err();
    assert_eq!(error, InlineImageError::Empty);
}

#[test]
fn rejects_oversized_decoded_payload_before_allocation() {
    let encoded = STANDARD.encode(PNG);
    assert_eq!(
        validate_inline_image("image/png", &encoded, PNG.len() - 1),
        Err(InlineImageError::DecodedByteLimitExceeded {
            actual_bytes: PNG.len(),
            max_bytes: PNG.len() - 1,
        })
    );
}

#[test]
fn rejects_unsupported_declared_mime_unknown_magic_and_mismatch() {
    let png = STANDARD.encode(PNG);
    assert_eq!(
        validate_inline_image("image/svg+xml", &png, PNG.len()),
        Err(InlineImageError::UnsupportedDeclaredMime)
    );

    let unknown = STANDARD.encode(b"not an image");
    assert_eq!(
        validate_inline_image("image/png", &unknown, 64),
        Err(InlineImageError::UnknownImageFormat)
    );

    assert_eq!(
        validate_inline_image("image/jpeg", &png, PNG.len()),
        Err(InlineImageError::MimeMismatch)
    );
}

#[test]
fn image_errors_do_not_include_encoded_content() {
    let encoded = STANDARD.encode(b"private-image-content");
    let error = validate_inline_image("image/png", &encoded, 64).unwrap_err();
    assert!(!format!("{error}").contains(&encoded));
    assert!(!format!("{error:?}").contains(&encoded));
}

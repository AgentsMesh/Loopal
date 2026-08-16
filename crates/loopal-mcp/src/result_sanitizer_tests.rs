use std::sync::Arc;

use loopal_secret_client::SecretString;
use rmcp::model::{CallToolResult, Content, RawContent, RawResource, ResourceContents};

use super::result_sanitizer::{BINARY_DENIED_MARKER, CallResultSanitizer};

fn sanitizer() -> CallResultSanitizer {
    CallResultSanitizer::new(&[("token".into(), SecretString::from("exact-plaintext"))])
}

#[test]
fn redacts_text_and_text_resource_with_exact_seed() {
    let result = CallToolResult::success(vec![
        Content::text("echo exact-plaintext"),
        Content::embedded_text("secret://exact-plaintext", "body exact-plaintext"),
    ]);
    let sanitized = sanitizer().sanitize(result);
    let rendered = serde_json::to_string(&sanitized).unwrap();
    assert!(!rendered.contains("exact-plaintext"));
    assert!(rendered.contains("<secret_ref:token>"));
}

#[test]
fn secret_bearing_image_audio_and_blob_are_replaced() {
    let audio: Content = serde_json::from_value(serde_json::json!({
        "type": "audio",
        "data": "ZXhhY3QtcGxhaW50ZXh0",
        "mimeType": "audio/wav"
    }))
    .unwrap();
    let result = CallToolResult::success(vec![
        Content::image("ZXhhY3QtcGxhaW50ZXh0", "image/png"),
        audio,
        Content::resource(ResourceContents::blob(
            "ZXhhY3QtcGxhaW50ZXh0",
            "secret://blob",
        )),
    ]);
    let sanitized = sanitizer().sanitize(result);
    assert_eq!(sanitized.content.len(), 3);
    for block in sanitized.content {
        assert!(matches!(
            block.raw,
            RawContent::Text(ref text) if text.text == BINARY_DENIED_MARKER
        ));
    }
}

#[test]
fn unknown_provenance_binary_is_denied_without_a_secret_seed() {
    let result = CallToolResult::success(vec![Content::image("c2FmZQ==", "image/png")]);
    let sanitized = CallResultSanitizer::new(&[]).sanitize(result);
    assert!(matches!(
        &sanitized.content[0].raw,
        RawContent::Text(text) if text.text == BINARY_DENIED_MARKER
    ));
}

#[test]
fn secret_bearing_structured_content_and_meta_do_not_cross_boundary() {
    let result = CallToolResult::structured(serde_json::json!({"echo": "exact-plaintext"}));
    let sanitized = sanitizer().sanitize(result);
    assert!(sanitized.structured_content.is_none());
    assert!(
        !serde_json::to_string(&sanitized)
            .unwrap()
            .contains("exact-plaintext")
    );
}

#[test]
fn secret_bearing_blob_resource_is_rejected() {
    assert!(sanitizer().reject_blob().is_err());
    assert!(CallResultSanitizer::new(&[]).reject_blob().is_err());
}

#[test]
fn redacts_resource_link_fields_and_resource_text() {
    let link = RawResource::new("secret://exact-plaintext", "exact-plaintext")
        .with_title("title exact-plaintext")
        .with_description("description exact-plaintext")
        .with_mime_type("type/exact-plaintext");
    let sanitized =
        sanitizer().sanitize(CallToolResult::success(vec![Content::resource_link(link)]));
    let encoded = serde_json::to_string(&sanitized).unwrap();
    assert!(!encoded.contains("exact-plaintext"));
    assert!(encoded.contains("<secret_ref:token>"));

    assert_eq!(
        sanitizer().sanitize_text("body exact-plaintext"),
        "body <secret_ref:token>"
    );
    assert_eq!(
        CallResultSanitizer::new(&[]).sanitize_text("ordinary"),
        "ordinary"
    );

    let link_without_optional_fields =
        RawResource::new("https://example.test/resource", "ordinary");
    let sanitized = sanitizer().sanitize(CallToolResult::success(vec![Content::resource_link(
        link_without_optional_fields,
    )]));
    let RawContent::ResourceLink(link) = &sanitized.content[0].raw else {
        panic!("safe resource link was not preserved")
    };
    assert!(link.title.is_none());
    assert!(link.description.is_none());
    assert!(link.mime_type.is_none());
}

#[test]
fn oauth_rotation_after_construction_is_redacted_dynamically() {
    let credentials = Arc::new(crate::oauth_credential_seed::OAuthCredentialSeed::default());
    credentials.observe(Some("initial-token")).unwrap();
    let sanitizer = CallResultSanitizer::with_oauth_credentials(&[], credentials.clone());
    credentials.observe(Some("rotated-token")).unwrap();

    assert_eq!(
        sanitizer.sanitize_text("initial-token Bearer rotated-token"),
        "<secret_ref:mcp_oauth_access_token> <secret_ref:mcp_oauth_bearer>"
    );
    credentials.deactivate();
    assert_eq!(
        sanitizer.sanitize_text("initial-token"),
        crate::oauth_credential_seed::OAUTH_RESPONSE_DENIED
    );
}

#[test]
fn embedded_resource_uris_are_denied_before_crossing_the_boundary() {
    for content in [
        Content::resource_link(RawResource::new(
            "data:image/png;base64,ZXhhY3QtcGxhaW50ZXh0",
            "image",
        )),
        Content::resource_link(RawResource::new(" BLOB:https://example.test/id", "blob")),
        Content::embedded_text("Data:text/plain,opaque", "ordinary"),
    ] {
        let sanitized = sanitizer().sanitize(CallToolResult::success(vec![content]));
        assert!(matches!(
            &sanitized.content[0].raw,
            RawContent::Text(text) if text.text == BINARY_DENIED_MARKER
        ));
        assert!(
            !serde_json::to_string(&sanitized)
                .unwrap()
                .contains("ZXhhY3Q")
        );
    }
}

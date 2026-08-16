use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use loopal_tool_api::{ImageOutputPolicy, ToolResult};
use loopal_tool_invocation::ToolImageBlock;
use secrecy::SecretString;

use super::{MAX_TOOL_IMAGES, finalize};

const PNG: &[u8] = b"\x89PNG\r\n\x1a\nbody";

fn image(bytes: &[u8]) -> ToolImageBlock {
    ToolImageBlock::inline("image/png", STANDARD.encode(bytes))
}

#[test]
fn final_text_is_redacted_and_bounded() {
    let seed = vec![(
        "token".into(),
        SecretString::from("plaintext-canary".to_string()),
    )];
    let result = finalize(
        "Test",
        ToolResult::success("plaintext-canary"),
        &seed,
        "session",
        ImageOutputPolicy::Deny,
        64,
    )
    .unwrap();
    assert_eq!(result.content, "<secret_ref:token>");

    let oversized = ToolResult::success("x".repeat(loopal_tool_api::DEFAULT_MAX_OUTPUT_BYTES + 1));
    assert!(
        finalize(
            "Test",
            oversized,
            &[],
            "session",
            ImageOutputPolicy::Deny,
            64,
        )
        .is_err()
    );
}

#[test]
fn image_capability_and_inline_shape_are_required() {
    let result = ToolResult::success("").with_images(vec![image(PNG)]);
    assert!(
        finalize(
            "Test",
            result.clone(),
            &[],
            "session",
            ImageOutputPolicy::Deny,
            64,
        )
        .is_err()
    );
    finalize(
        "Test",
        result,
        &[],
        "session",
        ImageOutputPolicy::ValidatedInline,
        64,
    )
    .unwrap();

    let resource = ToolResult::success("").with_images(vec![ToolImageBlock::session_resource(
        "id",
        "image/png",
        PNG.len(),
    )]);
    assert!(
        finalize(
            "Test",
            resource,
            &[],
            "session",
            ImageOutputPolicy::ValidatedInline,
            64,
        )
        .is_err()
    );
}

#[test]
fn image_count_format_and_total_bytes_are_bounded() {
    let too_many =
        ToolResult::success("").with_images((0..=MAX_TOOL_IMAGES).map(|_| image(PNG)).collect());
    assert!(
        finalize(
            "Test",
            too_many,
            &[],
            "session",
            ImageOutputPolicy::ValidatedInline,
            1024,
        )
        .is_err()
    );

    let malformed = ToolResult::success("")
        .with_images(vec![ToolImageBlock::inline("image/png", "not-base64")]);
    assert!(
        finalize(
            "Test",
            malformed,
            &[],
            "session",
            ImageOutputPolicy::ValidatedInline,
            64,
        )
        .is_err()
    );

    let total = ToolResult::success("").with_images(vec![image(PNG), image(PNG)]);
    assert!(
        finalize(
            "Test",
            total,
            &[],
            "session",
            ImageOutputPolicy::ValidatedInline,
            (PNG.len() + 1) as u64,
        )
        .is_err()
    );
}

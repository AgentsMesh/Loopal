use loopal_error::{LoopalError, Result, ToolError};
use loopal_output_guard::{OutputGuard, validate_inline_image};
use loopal_tool_api::{DEFAULT_MAX_OUTPUT_BYTES, ImageOutputPolicy, ToolResult, handle_overflow};
use loopal_tool_invocation::ToolImageBlock;
use secrecy::SecretString;

use crate::image_limits::{MAX_TOOL_IMAGES, image_byte_limit};

const MAX_RESULT_LINES: usize = 2000;
const MAX_RESULT_BYTES: usize = 100_000;

pub(crate) fn finalize(
    tool_name: &str,
    mut result: ToolResult,
    seed: &[(String, SecretString)],
    session_id: &str,
    image_policy: ImageOutputPolicy,
    configured_image_bytes: u64,
) -> Result<ToolResult> {
    result.content =
        loopal_secret_runtime::apply_redactor(tool_name, result.content, seed, session_id);
    let guard = match OutputGuard::new(seed) {
        Ok(guard) => guard,
        Err(_) => return Err(rejected("secret redactor unavailable")),
    };
    let guarded = match guard.guard_text(&result.content, DEFAULT_MAX_OUTPUT_BYTES) {
        Ok(content) => content.into_inner(),
        Err(_) => return Err(rejected("text exceeds final byte limit")),
    };
    validate_images(
        &result.images,
        image_policy,
        image_byte_limit(configured_image_bytes),
    )?;
    let overflow = handle_overflow(&guarded, MAX_RESULT_LINES, MAX_RESULT_BYTES, tool_name)
        .map_err(|_| rejected("overflow persistence failed"))?;
    result.content = overflow.display;
    Ok(result)
}

fn validate_images(
    images: &[ToolImageBlock],
    policy: ImageOutputPolicy,
    max_bytes: usize,
) -> Result<()> {
    if images.is_empty() {
        return Ok(());
    }
    if policy != ImageOutputPolicy::ValidatedInline {
        return Err(rejected("image output is not authorized for this tool"));
    }
    if images.len() > MAX_TOOL_IMAGES {
        return Err(rejected("image count exceeds final limit"));
    }
    let mut total_bytes = 0usize;
    for image in images {
        let ToolImageBlock::Inline { media_type, data } = image else {
            return Err(rejected("tool returned an untrusted resource reference"));
        };
        let validated = match validate_inline_image(media_type, data, max_bytes) {
            Ok(validated) => validated,
            Err(_) => return Err(rejected("inline image validation failed")),
        };
        total_bytes = total_bytes.saturating_add(validated.byte_size());
        if total_bytes > max_bytes {
            return Err(rejected("image bytes exceed final limit"));
        }
    }
    Ok(())
}

fn rejected(reason: &str) -> LoopalError {
    ToolError::ExecutionFailed(format!("tool result rejected: {reason}")).into()
}

#[cfg(test)]
#[path = "tool_result_guard/tests.rs"]
mod tests;

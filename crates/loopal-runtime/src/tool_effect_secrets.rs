use loopal_error::Result;
use loopal_tool_api::ToolContext;
use secrecy::SecretString;

use crate::tool_pipeline::integrity_error;

pub(crate) async fn resolve_effect_secrets(
    tool_name: &str,
    input: &mut serde_json::Value,
    eligible: &[&str],
    ctx: &ToolContext,
) -> Result<Vec<(String, SecretString)>> {
    let refs = loopal_secret_runtime::collect_wire_refs(input, eligible);
    if refs.is_empty() {
        return Ok(Vec::new());
    }
    let seed = loopal_secret_runtime::apply_resolver(
        tool_name,
        input,
        eligible,
        ctx.secret_client.as_ref(),
        &ctx.session_id,
    )
    .await;
    if !loopal_secret_runtime::collect_wire_refs(input, eligible).is_empty() {
        return Err(integrity_error("secret resolution failed".into()));
    }
    Ok(seed)
}

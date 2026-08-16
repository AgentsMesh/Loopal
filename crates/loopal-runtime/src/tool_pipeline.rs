use std::time::Instant;

use loopal_config::HookEvent;
use loopal_error::{LoopalError, Result, ToolError};
use loopal_hooks::{HookContext, HookOutput};
use loopal_kernel::Kernel;
use loopal_tool_api::{PermissionLevel, ToolContext, ToolResult};
use opentelemetry::KeyValue;
use tracing::{Instrument, debug, info};

use crate::mode::AgentMode;
use crate::tool_action::PreparedToolAction;
use crate::tool_execution_output::ToolExecutionOutput;
use crate::tool_input_validation::{validate_tool_input, validate_wire_refs};

pub async fn execute_tool(
    kernel: &Kernel,
    action: PreparedToolAction,
    ctx: &ToolContext,
    mode: &AgentMode,
) -> Result<ToolResult> {
    let name = action.tool_name().to_string();
    let output = execute_tool_output(kernel, action, ctx, mode).await?;
    let result = output.outcome?;
    crate::tool_result_guard::finalize(
        &name,
        result,
        &output.seed,
        &ctx.session_id,
        output.image_policy,
        kernel.settings().images.max_bytes,
    )
}

pub(crate) async fn execute_tool_output(
    kernel: &Kernel,
    action: PreparedToolAction,
    ctx: &ToolContext,
    _mode: &AgentMode,
) -> Result<ToolExecutionOutput> {
    action.verify(kernel)?;
    let name = action.tool_name().to_string();
    let placeholder_input = action.placeholder_input().clone();
    let mut effective_input = placeholder_input.clone();
    let tool = action.tool().clone();
    validate_effect_input(tool.as_ref(), &placeholder_input)?;
    if tool.permission() != PermissionLevel::ReadOnly {
        let audit = ctx.protected_effect_audit.as_ref().ok_or_else(|| {
            integrity_error("protected effect audit capability unavailable".into())
        })?;
        let request = action.protected_effect_audit_request()?;
        audit
            .record(&request)
            .await
            .map_err(|error| integrity_error(format!("protected effect audit failed: {error}")))?;
    }

    let seed = crate::tool_effect_secrets::resolve_effect_secrets(
        &name,
        &mut effective_input,
        tool.secret_eligible_params(),
        ctx,
    )
    .await?;
    let image_policy = tool.image_output_policy();
    let mut effect_ctx = ctx.clone();
    effect_ctx.process_executor = None;
    if !seed.is_empty() {
        let sanitizer = std::sync::Arc::new(
            crate::process_output_sanitizer::SecretProcessOutputSanitizer::new(
                &name,
                &ctx.session_id,
                &seed,
            )?,
        );
        effect_ctx.process_executor = Some(std::sync::Arc::new(
            crate::process_guarded_backend::GuardedProcessExecutor::new(
                effect_ctx.backend.clone(),
                sanitizer,
            ),
        ));
    }
    let tool_span = tracing::info_span!("tool_exec", tool.name = name.as_str());
    let outcome = async {
        debug!(tool = name.as_str(), "executing tool");
        let start = Instant::now();
        let outcome = tool.execute(effective_input, &effect_ctx).await;
        let duration = start.elapsed();
        let ok = outcome.as_ref().is_ok_and(|result| !result.is_error);
        let output_len = outcome.as_ref().map_or(0, |result| result.content.len());
        info!(
            tool = name.as_str(),
            duration_ms = duration.as_millis() as u64,
            ok,
            output_len,
            "tool pipeline exec"
        );
        let tool_attrs = &[KeyValue::new("tool.name", name.clone())];
        crate::otel_metrics::tool_duration().record(duration.as_secs_f64(), tool_attrs);
        crate::otel_metrics::tool_invocations().add(1, tool_attrs);
        outcome
    }
    .instrument(tool_span)
    .await;

    let mut result = match outcome {
        Ok(result) => result,
        Err(error) => {
            let message = loopal_secret_runtime::apply_redactor(
                &name,
                error.to_string(),
                &seed,
                &ctx.session_id,
            );
            return Ok(ToolExecutionOutput {
                outcome: Err(LoopalError::Other(message)),
                seed,
                image_policy,
            });
        }
    };
    result.content =
        loopal_secret_runtime::apply_redactor(&name, result.content, &seed, &ctx.session_id);
    let post_outputs = kernel
        .hook_service()
        .run_hooks(
            HookEvent::PostToolUse,
            &HookContext {
                tool_name: Some(&name),
                tool_input: Some(&placeholder_input),
                tool_output: Some(&result.content),
                is_error: Some(result.is_error),
                ..Default::default()
            },
        )
        .await;
    result = append_post_hook_feedback(result, &post_outputs);
    Ok(ToolExecutionOutput {
        outcome: Ok(result),
        seed,
        image_policy,
    })
}

fn validate_effect_input(
    tool: &dyn loopal_tool_api::Tool,
    input: &serde_json::Value,
) -> Result<()> {
    if let Err(reason) = validate_tool_input(tool, input) {
        return Err(integrity_error(format!(
            "schema validation failed: {reason}"
        )));
    }
    if let Err(reason) = validate_wire_refs(input, tool.secret_eligible_params()) {
        return Err(integrity_error(format!(
            "wire-ref validation failed: {reason}"
        )));
    }
    if let Some(reason) = tool.precheck(input) {
        return Err(integrity_error(format!("tool precheck failed: {reason}")));
    }
    Ok(())
}

pub(crate) fn integrity_error(reason: String) -> LoopalError {
    ToolError::InvalidInput(format!("approved tool action integrity mismatch: {reason}")).into()
}

fn append_post_hook_feedback(mut result: ToolResult, outputs: &[HookOutput]) -> ToolResult {
    let feedback: Vec<&str> = outputs
        .iter()
        .filter_map(|output| output.additional_context.as_deref())
        .collect();
    if !feedback.is_empty() {
        result.content.push_str("\n\n[POST-HOOK FEEDBACK]\n");
        result.content.push_str(&feedback.join("\n---\n"));
    }
    result
}

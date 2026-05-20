use loopal_tool_api::{AppliedKind, BatchOp, BatchOutcome, ToolContext, ToolResult};

pub async fn commit_patch(ops: Vec<BatchOp>, ctx: &ToolContext) -> Result<ToolResult, String> {
    let outcome = ctx
        .backend
        .apply_batch(ops)
        .await
        .map_err(|e| format!("apply_batch failed: {e}"))?;
    Ok(format_outcome(outcome))
}

fn format_outcome(outcome: BatchOutcome) -> ToolResult {
    let (mut created, mut updated, mut deleted) = (0u32, 0u32, 0u32);
    for applied in &outcome.applied {
        match applied.kind {
            AppliedKind::Created => created += 1,
            AppliedKind::Updated => updated += 1,
            AppliedKind::Deleted => deleted += 1,
        }
    }
    let mut parts = Vec::new();
    if updated > 0 {
        parts.push(format!("{updated} updated"));
    }
    if created > 0 {
        parts.push(format!("{created} created"));
    }
    if deleted > 0 {
        parts.push(format!("{deleted} deleted"));
    }
    let summary = if parts.is_empty() {
        "(no changes)".to_string()
    } else {
        format!("Applied: {}", parts.join(", "))
    };

    match outcome.failed_at {
        Some(failed) => {
            let applied_paths: Vec<String> = outcome
                .applied
                .iter()
                .map(|a| a.path.display().to_string())
                .collect();
            ToolResult::error(format!(
                "{summary}; failed at op {} ({}): {}; already applied: [{}]",
                failed.index,
                failed.path.display(),
                failed.error,
                applied_paths.join(", ")
            ))
        }
        None => ToolResult::success(summary),
    }
}

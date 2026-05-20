use std::collections::HashMap;

use loopal_edit_core::hunks::apply_hunks_to_text;
use loopal_edit_core::omission_detector::detect_omissions;
use loopal_edit_core::patch_types::FileOp;
use loopal_tool_api::{BatchOp, BatchWriteKind, ResolvedPath, ToolContext};

#[derive(Debug)]
pub struct PlanError {
    pub message: String,
}

enum FileState {
    Write { content: String, was_existing: bool },
    Delete,
}

pub async fn plan_patch(ops: &[FileOp], ctx: &ToolContext) -> Result<Vec<BatchOp>, PlanError> {
    let mut state: HashMap<ResolvedPath, FileState> = HashMap::new();
    let mut order: Vec<ResolvedPath> = Vec::new();

    for op in ops {
        let raw = op.path().to_string_lossy().into_owned();
        let resolved = ctx
            .backend
            .resolve_path(&raw, true)
            .map_err(|e| PlanError {
                message: format!("{raw}: {e}"),
            })?;

        let initial_exists = state.contains_key(&resolved)
            || ctx
                .backend
                .file_info(&resolved)
                .await
                .map(|_| true)
                .unwrap_or(false);

        match op {
            FileOp::Add { path, content } => {
                let exists_now = match state.get(&resolved) {
                    Some(FileState::Delete) => false,
                    Some(FileState::Write { .. }) => true,
                    None => initial_exists,
                };
                if exists_now {
                    return Err(PlanError {
                        message: format!("{}: file already exists", path.display()),
                    });
                }
                let om = detect_omissions(content);
                if !om.is_empty() {
                    return Err(PlanError {
                        message: format!(
                            "{}: omission detected: {}",
                            path.display(),
                            om.join(", ")
                        ),
                    });
                }
                upsert(
                    &mut state,
                    &mut order,
                    resolved,
                    FileState::Write {
                        content: content.clone(),
                        was_existing: initial_exists,
                    },
                );
            }
            FileOp::Delete { path } => {
                match state.get(&resolved) {
                    Some(FileState::Delete) => {
                        return Err(PlanError {
                            message: format!("{}: already deleted in this patch", path.display()),
                        });
                    }
                    Some(FileState::Write { .. }) => {}
                    None if !initial_exists => {
                        return Err(PlanError {
                            message: format!("{}: file does not exist", path.display()),
                        });
                    }
                    None => {}
                }
                upsert(&mut state, &mut order, resolved, FileState::Delete);
            }
            FileOp::Update { path, hunks } => {
                let base = base_for_update(&state, &resolved, ctx, path).await?;
                let updated = apply_hunks_to_text(&base, hunks).map_err(|e| PlanError {
                    message: format!("{}: {e}", path.display()),
                })?;
                upsert(
                    &mut state,
                    &mut order,
                    resolved,
                    FileState::Write {
                        content: updated,
                        was_existing: true,
                    },
                );
            }
        }
    }

    let mut ops_out = Vec::new();
    for path in order {
        match state.remove(&path).unwrap() {
            FileState::Write {
                content,
                was_existing,
            } => {
                ops_out.push(BatchOp::Write {
                    path,
                    content,
                    expected_kind: if was_existing {
                        BatchWriteKind::Update
                    } else {
                        BatchWriteKind::Create
                    },
                });
            }
            FileState::Delete => ops_out.push(BatchOp::Delete { path }),
        }
    }
    Ok(ops_out)
}

async fn base_for_update(
    state: &HashMap<ResolvedPath, FileState>,
    resolved: &ResolvedPath,
    ctx: &ToolContext,
    path: &std::path::Path,
) -> Result<String, PlanError> {
    match state.get(resolved) {
        Some(FileState::Delete) => Err(PlanError {
            message: format!(
                "{}: cannot update file deleted earlier in this patch",
                path.display()
            ),
        }),
        Some(FileState::Write { .. }) => Err(PlanError {
            message: format!(
                "{}: cannot update file added/modified earlier in this patch (not supported in v1)",
                path.display()
            ),
        }),
        None => ctx.backend.read_raw(resolved).await.map_err(|e| PlanError {
            message: format!("{}: cannot read: {e}", path.display()),
        }),
    }
}

fn upsert(
    state: &mut HashMap<ResolvedPath, FileState>,
    order: &mut Vec<ResolvedPath>,
    key: ResolvedPath,
    value: FileState,
) {
    if !state.contains_key(&key) {
        order.push(key.clone());
    }
    state.insert(key, value);
}

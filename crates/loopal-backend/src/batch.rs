use loopal_tool_api::{AppliedKind, AppliedOp, BatchOp, BatchOutcome, BatchWriteKind, FailedOp};

use crate::fs::{remove_path, write_file};

pub async fn apply_batch(ops: Vec<BatchOp>) -> BatchOutcome {
    let mut applied = Vec::new();
    for (index, op) in ops.into_iter().enumerate() {
        match op {
            BatchOp::Write {
                path,
                content,
                expected_kind,
            } => {
                let path_buf = path.into_path_buf();
                match write_file(&path_buf, &content).await {
                    Ok(_) => applied.push(AppliedOp {
                        path: path_buf,
                        kind: applied_kind_for(expected_kind),
                    }),
                    Err(e) => {
                        return BatchOutcome {
                            applied,
                            failed_at: Some(FailedOp {
                                index,
                                path: path_buf,
                                error: e,
                            }),
                        };
                    }
                }
            }
            BatchOp::Delete { path } => {
                let path_buf = path.into_path_buf();
                if let Err(e) = remove_path(&path_buf).await {
                    return BatchOutcome {
                        applied,
                        failed_at: Some(FailedOp {
                            index,
                            path: path_buf,
                            error: e,
                        }),
                    };
                }
                applied.push(AppliedOp {
                    path: path_buf,
                    kind: AppliedKind::Deleted,
                });
            }
        }
    }
    BatchOutcome {
        applied,
        failed_at: None,
    }
}

fn applied_kind_for(kind: BatchWriteKind) -> AppliedKind {
    match kind {
        BatchWriteKind::Create => AppliedKind::Created,
        BatchWriteKind::Update => AppliedKind::Updated,
    }
}

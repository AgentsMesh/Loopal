use std::path::Path;

use super::contextualize;
use crate::workflow_journal::error::WorkflowJournalError;

#[test]
fn contextualize_rewrites_only_corruption_locations() {
    let path = Path::new("runs/replay.jsonl");
    let contextualized = contextualize(
        WorkflowJournalError::Corruption {
            path: Default::default(),
            offset: 0,
            detail: "invalid record".into(),
        },
        path,
        41,
    );
    assert!(matches!(
        contextualized,
        WorkflowJournalError::Corruption {
            path: actual,
            offset: 41,
            detail,
        } if actual == path && detail == "invalid record"
    ));

    let untouched = contextualize(
        WorkflowJournalError::InvalidRunId("../bad".into()),
        path,
        42,
    );
    assert!(matches!(
        untouched,
        WorkflowJournalError::InvalidRunId(value) if value == "../bad"
    ));
}

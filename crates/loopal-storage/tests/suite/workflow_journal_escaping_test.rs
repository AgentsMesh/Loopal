use loopal_protocol::{
    AgentCompletion, MAX_WORKFLOW_OUTPUT_BYTES, WorkflowEvent, WorkflowEventPayload, WorkflowOutput,
};
use loopal_storage::MAX_WORKFLOW_JOURNAL_LINE_BYTES;

use crate::workflow_journal_support::{journal, path, snapshot};

#[test]
fn worst_case_escaped_completion_and_output_fit_and_replay() {
    let temp = tempfile::tempdir().unwrap();
    let journal = journal(&temp);
    let mut initial = snapshot();
    initial.spec.limits.max_output_bytes = MAX_WORKFLOW_OUTPUT_BYTES;
    initial.spec.output_contract = loopal_protocol::WorkflowOutputContract::Text {
        max_bytes: MAX_WORKFLOW_OUTPUT_BYTES,
    };
    journal.append_init(initial).unwrap();
    let escaped = "\0".repeat(MAX_WORKFLOW_OUTPUT_BYTES as usize);
    let event = WorkflowEvent {
        run_id: "wrun_test".into(),
        revision: 1,
        occurred_at_unix_ms: 101,
        payload: WorkflowEventPayload::AttemptSucceeded {
            node_id: "output".into(),
            attempt_id: "watt_output".into(),
            completion: AgentCompletion::goal(Some(escaped.clone())),
            output: Some(WorkflowOutput::Text(escaped.clone())),
        },
    };

    journal.append_commit(vec![event], None).unwrap();

    let bytes = std::fs::read(path(&temp)).unwrap();
    let lines: Vec<&[u8]> = bytes.split(|byte| *byte == b'\n').collect();
    assert_eq!(lines.len(), 3);
    assert!(lines[1].len() <= MAX_WORKFLOW_JOURNAL_LINE_BYTES);
    assert!(lines[1].len() > MAX_WORKFLOW_OUTPUT_BYTES as usize * 10);

    let replay = journal.replay().unwrap();
    let WorkflowEventPayload::AttemptSucceeded {
        completion, output, ..
    } = &replay.commits[0].events[0].payload
    else {
        panic!("expected attempt success");
    };
    assert_eq!(completion.result.as_deref(), Some(escaped.as_str()));
    assert_eq!(output, &Some(WorkflowOutput::Text(escaped)));
}

use loopal_protocol::{
    MAX_JSON_SCHEMA_BYTES, MAX_WORKFLOW_GOAL_BYTES, MAX_WORKFLOW_ID_BYTES,
    MAX_WORKFLOW_OUTPUT_BYTES, MAX_WORKFLOW_TASK_BYTES, WorkflowNodeId, WorkflowOutputContract,
};

use super::requests::{causation, request};
use crate::workflow::scheduler::WorkflowDependencyResult;

#[test]
fn text_output_prompt_carries_ordered_escaped_dependency_results_and_exact_limit() {
    let mut request = request(causation("wrun", "join", "watt"));
    request.dependency_results = vec![
        dependency("left", "left-result"),
        dependency("right", "right\n\"result\""),
    ];
    request.output_contract = Some(WorkflowOutputContract::Text { max_bytes: 1_024 });

    let prompt = super::super::worker_prompt::build(&request);

    assert!(prompt.contains(
        "Authoritative dependency results (JSON, in declared dependency order):\n\
         [{\"node_id\":\"left\",\"result\":\"left-result\"},{\"node_id\":\"right\",\
         \"result\":\"right\\n\\\"result\\\"\"}]"
    ));
    assert!(prompt.contains("Return exactly one final plain-text value"));
    assert!(prompt.contains("must be no longer than 1024 bytes"));
    assert!(prompt.contains("without JSON encoding or Markdown fences"));
}

#[test]
fn json_output_prompt_uses_canonical_safely_serialized_schema() {
    let mut properties = serde_json::Map::new();
    properties.insert("z".into(), serde_json::json!({"type": "number"}));
    properties.insert(
        "a".into(),
        serde_json::json!({"description": "line one\nline two", "type": "string"}),
    );
    let mut schema = serde_json::Map::new();
    schema.insert("properties".into(), properties.into());
    schema.insert("additionalProperties".into(), false.into());
    schema.insert("type".into(), "object".into());
    let mut request = request(causation("wrun", "output", "watt"));
    request.output_contract = Some(WorkflowOutputContract::Json {
        max_bytes: 2_048,
        schema: schema.into(),
    });

    let prompt = super::super::worker_prompt::build(&request);

    assert!(prompt.contains("Return exactly one final JSON value"));
    assert!(prompt.contains("must be no longer than 2048 bytes"));
    assert!(prompt.contains(
        "Canonical JSON Schema:\n{\"additionalProperties\":false,\"properties\":{\"a\":{\
         \"description\":\"line one\\nline two\",\"type\":\"string\"},\"z\":{\"type\":\
         \"number\"}},\"type\":\"object\"}"
    ));
}

#[test]
fn prompt_handles_absent_sections_and_every_canonical_json_value_kind() {
    let plain_request = request(causation("wrun", "plain", "watt"));
    let prompt = super::super::worker_prompt::build(&plain_request);
    assert!(!prompt.contains("Authoritative dependency results"));
    assert!(!prompt.contains("Output contract (authoritative)"));

    let mut request = request(causation("wrun", "json", "watt"));
    request.output_contract = Some(WorkflowOutputContract::Json {
        max_bytes: 128,
        schema: serde_json::json!({"values": [null, true, 3, "four"]}),
    });
    let prompt = super::super::worker_prompt::build(&request);
    assert!(prompt.contains("{\"values\":[null,true,3,\"four\"]}"));
}

#[test]
fn maximum_dependency_prompt_fits_the_agent_start_frame() {
    let causation = causation(&max_id("r", 0), &max_id("n", 0), &max_id("a", 0));
    let mut request = request(causation.clone());
    request.run_goal = "\0".repeat(MAX_WORKFLOW_GOAL_BYTES);
    request.task = "\0".repeat(MAX_WORKFLOW_TASK_BYTES);
    let result = "\0".repeat(MAX_WORKFLOW_OUTPUT_BYTES as usize);
    request.dependency_results = (0..8)
        .map(|index| dependency(&max_id("d", index), &result))
        .collect();
    let schema_text = "\0".repeat((MAX_JSON_SCHEMA_BYTES - 64) / 6);
    request.output_contract = Some(WorkflowOutputContract::Json {
        max_bytes: MAX_WORKFLOW_OUTPUT_BYTES,
        schema: serde_json::json!({"description": schema_text, "type": "string"}),
    });

    let prompt = super::super::worker_prompt::build(&request);
    let params = loopal_agent_client::encode(&loopal_agent_client::StartAgentParams {
        cwd: std::path::PathBuf::from(format!("/{}", "c".repeat(32_000))),
        // Production JSON configuration is capped at 4 MiB including its wrapper.
        model: Some("m".repeat(4 * 1_024 * 1_024 - 64)),
        prompt: Some(prompt),
        workflow_permission_causation: Some(causation),
        workflow_attempt_capability: Some(request.attempt_capability),
        workflow_completion_result_limit: Some(MAX_WORKFLOW_OUTPUT_BYTES),
        session_id: Some("s".repeat(MAX_WORKFLOW_ID_BYTES)),
        depth: Some(u32::MAX),
        ..Default::default()
    });
    let frame = loopal_ipc::jsonrpc::encode_request(
        i64::MAX,
        loopal_ipc::methods::AGENT_START.name,
        params,
    );

    assert!(
        frame.len() > loopal_ipc::MAX_IPC_FRAME_BYTES - 4 * 1_024 * 1_024,
        "frame fixture no longer exercises the final 4 MiB of the IPC budget"
    );
    assert!(
        frame.len() <= loopal_ipc::MAX_IPC_FRAME_BYTES,
        "maximum legal workflow agent/start frame is {} bytes (limit {})",
        frame.len(),
        loopal_ipc::MAX_IPC_FRAME_BYTES,
    );
}

fn dependency(node_id: &str, result: &str) -> WorkflowDependencyResult {
    WorkflowDependencyResult {
        node_id: WorkflowNodeId::new(node_id),
        result: result.into(),
    }
}

fn max_id(prefix: &str, index: usize) -> String {
    let prefix = format!("{prefix}{index}");
    format!(
        "{prefix}{}",
        "x".repeat(MAX_WORKFLOW_ID_BYTES - prefix.len())
    )
}

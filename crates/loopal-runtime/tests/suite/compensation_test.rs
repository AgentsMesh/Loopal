use loopal_provider_api::{ContentBlock, MessageRole};
use loopal_tool_invocation::{CancelCause, ToolResultMetadata};
use serde_json::json;

use loopal_runtime::agent_loop::governance::synthesize_aborted_tool_results;

fn tool(id: &str, name: &str) -> (String, String, serde_json::Value) {
    (id.into(), name.into(), json!({"x": 1}))
}

#[test]
fn empty_tool_uses_returns_none() {
    assert!(synthesize_aborted_tool_results(&[], "loop detected").is_none());
}

#[test]
fn single_tool_use_produces_user_message_with_one_tool_result() {
    let uses = [tool("u1", "Bash")];
    let msg = synthesize_aborted_tool_results(&uses, "loop detected").unwrap();
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.id, None);
    assert_eq!(msg.content.len(), 1);
    let ContentBlock::ToolResult {
        tool_use_id,
        content,
        is_error,
        metadata,
        ..
    } = &msg.content[0]
    else {
        panic!("expected ToolResult");
    };
    assert_eq!(tool_use_id, "u1");
    assert!(content.contains("loop detected"));
    assert!(content.contains("Bash"));
    assert!(is_error);
    assert_eq!(
        metadata.as_ref().unwrap(),
        &ToolResultMetadata::Cancelled {
            cause: CancelCause::GovernanceAbort
        }
    );
}

#[test]
fn multiple_tool_uses_produce_one_message_with_all_blocks() {
    let uses = [tool("u1", "Bash"), tool("u2", "Read"), tool("u3", "Edit")];
    let msg = synthesize_aborted_tool_results(&uses, "policy violation").unwrap();
    assert_eq!(msg.role, MessageRole::User);
    assert_eq!(msg.content.len(), 3);
    let ids: Vec<&str> = msg
        .content
        .iter()
        .map(|b| match b {
            ContentBlock::ToolResult { tool_use_id, .. } => tool_use_id.as_str(),
            _ => panic!("expected ToolResult"),
        })
        .collect();
    assert_eq!(ids, vec!["u1", "u2", "u3"]);
    for b in &msg.content {
        let ContentBlock::ToolResult {
            is_error,
            metadata,
            content,
            ..
        } = b
        else {
            panic!("expected ToolResult");
        };
        assert!(is_error);
        assert!(content.contains("policy violation"));
        assert!(matches!(
            metadata.as_ref().unwrap(),
            ToolResultMetadata::Cancelled {
                cause: CancelCause::GovernanceAbort
            }
        ));
    }
}

#[test]
fn cancel_cause_serializes_as_governance_abort() {
    let md = ToolResultMetadata::Cancelled {
        cause: CancelCause::GovernanceAbort,
    };
    let wire = serde_json::to_value(&md).unwrap();
    assert_eq!(
        wire,
        json!({"kind": "cancelled", "cause": "governance_abort"})
    );
}

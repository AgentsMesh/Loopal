use std::fmt::Write;

use loopal_protocol::WorkflowOutputContract;
use serde_json::Value;

use crate::workflow::scheduler::WorkflowSpawnRequest;

pub(super) fn build(request: &WorkflowSpawnRequest) -> String {
    let mut prompt = format!(
        "Workflow goal:\n{}\n\nAssigned task:\n{}\n\n\
         Completion result requirement:\n\
         Return the task's authoritative result in the final agent completion result field. \
         Downstream workflow nodes receive only that result; do not leave it absent.",
        request.run_goal, request.task
    );
    if !request.dependency_results.is_empty() {
        prompt.push_str(
            "\n\nAuthoritative dependency results (JSON, in declared dependency order):\n",
        );
        let values: Vec<_> = request
            .dependency_results
            .iter()
            .map(|dependency| {
                serde_json::json!({
                    "node_id": dependency.node_id.as_str(),
                    "result": dependency.result.as_str(),
                })
            })
            .collect();
        prompt.push_str(
            &serde_json::to_string(&values)
                .expect("workflow dependency result JSON serialization cannot fail"),
        );
    }
    if let Some(contract) = &request.output_contract {
        append_output_contract(&mut prompt, contract);
    }
    prompt
}

fn append_output_contract(prompt: &mut String, contract: &WorkflowOutputContract) {
    match contract {
        WorkflowOutputContract::Text { max_bytes } => {
            write!(
                prompt,
                "\n\nOutput contract (authoritative):\n\
                 Return exactly one final plain-text value in the agent completion result field. \
                 The UTF-8 result must be no longer than {max_bytes} bytes. \
                 Return only that text, without JSON encoding or Markdown fences."
            )
            .expect("writing to String cannot fail");
        }
        WorkflowOutputContract::Json { max_bytes, schema } => {
            write!(
                prompt,
                "\n\nOutput contract (authoritative):\n\
                 Return exactly one final JSON value in the agent completion result field. \
                 The UTF-8 JSON result must be no longer than {max_bytes} bytes and must satisfy \
                 the canonical JSON Schema below. Return only the JSON value, without Markdown \
                 fences or commentary.\nCanonical JSON Schema:\n{}",
                canonical_json(schema)
            )
            .expect("writing to String cannot fail");
        }
    }
}

fn canonical_json(value: &Value) -> String {
    let mut output = String::new();
    write_canonical(value, &mut output);
    output
}

fn write_canonical(value: &Value, output: &mut String) {
    match value {
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => output.push_str(
            &serde_json::to_string(value).expect("JSON scalar serialization cannot fail"),
        ),
        Value::Array(values) => {
            output.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                write_canonical(value, output);
            }
            output.push(']');
        }
        Value::Object(values) => {
            output.push('{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(',');
                }
                output.push_str(
                    &serde_json::to_string(key).expect("JSON object key serialization cannot fail"),
                );
                output.push(':');
                write_canonical(&values[key], output);
            }
            output.push('}');
        }
    }
}

use super::workspace_rpc_support::setup;
use loopal_ipc::protocol::methods;
use serde_json::json;

fn settings(model: &str) -> serde_json::Value {
    json!({
        "model": model,
        "modelRouting": {
            "default": "", "summarization": "", "classification": "classifier-model",
            "refine": ""
        },
        "permissionMode": "ask_dangerous",
        "decisionMode": "classifier",
        "sandboxPolicy": "read_only",
        "thinking": {"type": "effort", "level": "high"},
        "maxContextTokens": 180000,
        "memoryEnabled": false,
        "microcompactIdleMinutes": 15,
        "telemetryEnabled": false,
        "outputStyle": "engineer"
    })
}

include!("desktop_settings_rpc_test/persistence.rs");
include!("desktop_settings_rpc_test/compatible.rs");
include!("desktop_settings_rpc_test/validation_acl.rs");

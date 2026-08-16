use serde_json::{Value, json};

use crate::support::HubHarness;

#[tokio::test]
async fn protected_effect_audit_is_fsynced_before_bash_effect() {
    let mut h = HubHarness::start(json!({
        "version": 2,
        "name": "protected_audit",
        "calls": [
            {"expect": {"userContains": "run audited effect"},
             "chunks": [
                {"type": "tool_use", "id": "audit-call-1", "name": "Bash",
                 "input": {"command":
                    "audit=\"$HOME/.loopal/telemetry/secret_access.jsonl\"; \
                     grep -q '\"op\":\"tool_effect\".*\"tool_call_id\":\"audit-call-1\"' \"$audit\" && \
                     printf '%s' audit-effect-canary > audit-effect-marker"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "audit-call-1"},
             "chunks": [
                {"type": "text", "text": "audited effect complete"},
                {"type": "done"}
             ]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;

    let out = h.turn("run audited effect").await;
    assert!(
        out.error.is_none() && out.finished && out.text.contains("audited effect complete"),
        "turn failed: {out:?}"
    );
    assert_eq!(
        std::fs::read_to_string(h.cwd().join("audit-effect-marker")).unwrap(),
        "audit-effect-canary"
    );

    let lines = std::fs::read_to_string(h.protected_audit_path()).unwrap();
    let records = lines
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .filter(|value| value["tool_call_id"] == "audit-call-1")
        .collect::<Vec<_>>();
    let effect = records
        .iter()
        .find(|value| value["op"] == "tool_effect")
        .expect("protected effect audit record");
    let permission = records
        .iter()
        .find(|value| value["op"] == "permission_decision")
        .expect("permission decision audit record");

    assert_eq!(effect["phase"], "pre_effect");
    assert_eq!(effect["name"], "audit-call-1");
    assert_eq!(permission["decision"], "allow");
    assert_eq!(permission["decision_source"], "policy");
    for record in [effect, permission] {
        assert_eq!(record["tool_name"], "Bash");
        assert_eq!(record["session_id"], h.session_id);
        assert_eq!(record["agent_name"], "main");
        assert_eq!(record["depth"], 0);
        assert!(record["connection_generation"].as_u64().is_some());
        for field in ["action_digest", "schema_digest"] {
            assert!(record[field].as_str().unwrap().starts_with("sha256:"));
        }
        assert!(record.get("action_input").is_none());
    }
    assert!(!lines.contains("audit-effect-canary"));
    assert!(!lines.contains("audit-effect-marker"));
}

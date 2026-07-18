use std::time::Duration;

use serde_json::json;

use crate::support::CliHarness;

/// Hub secret outage through a live session: three failed `hub/secret/get`
/// resolutions trip HubHealth's degrade threshold, the health poller emits
/// HubDegraded over the wire, yet the turn completes gracefully — unresolved
/// refs are rewritten to `<missing-secret:NAME>` markers instead of crashing.
/// After the vault heals, the next successful resolution emits HubRecovered.
#[tokio::test]
async fn hub_outage_degrades_and_recovers_over_the_wire() {
    let mut h = CliHarness::start(json!({
        "version": 2,
        "name": "hub_degraded",
        "calls": [
            {"expect": {"userContains": "trigger outage"},
             "chunks": [
                {"type": "tool_use", "id": "d1", "name": "Bash",
                 "input": {"command":
                    "echo 'o-<secret_ref:dg_a>-o' 'o-<secret_ref:dg_b>-o' 'o-<secret_ref:dg_c>-o'"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "d1"},
             "chunks": [{"type": "text", "text": "outage turn done"}, {"type": "done"}]},
            {"expect": {"userContains": "after recovery"},
             "chunks": [
                {"type": "tool_use", "id": "d2", "name": "Bash",
                 "input": {"command": "echo 'r-<secret_ref:dg_a>-r'"}},
                {"type": "done"}
             ]},
            {"expect": {"toolResultId": "d2"},
             "chunks": [{"type": "text", "text": "recovery turn done"}, {"type": "done"}]}
        ],
        "fallback": {"chunks": [{"type": "text", "text": "fallback"}, {"type": "done"}]}
    }))
    .await;
    h.vault().insert("dg_a", "dg-recovered-plain");
    h.vault().set_failing(true);
    h.begin_persistent().await;

    let out1 = h.turn_via_message("please trigger outage").await;
    assert!(
        out1.finished && out1.error.is_none(),
        "the turn must survive a vault outage; out: {out1:?}"
    );
    assert!(
        out1.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("<missing-secret:dg_b>")),
        "unresolved refs must be rewritten to missing-secret markers, not \
         crash the turn; events: {:?}",
        out1.events
    );
    let degraded = out1.events.iter().any(|e| e.contains("HubDegraded"))
        || h.await_event("HubDegraded", Duration::from_secs(3)).await;
    assert!(
        degraded,
        "three failed resolutions must surface a HubDegraded event; \
         turn events: {:?}",
        out1.events
    );

    h.vault().set_failing(false);
    let out2 = h.turn_via_message("now after recovery").await;
    assert!(
        out2.finished && out2.text.contains("recovery turn done"),
        "recovery turn failed: {out2:?}"
    );
    assert!(
        out2.events
            .iter()
            .any(|e| e.starts_with("ToolResult") && e.contains("r-<secret_ref:dg_a>-r")),
        "the healed resolution must inject plaintext and redact it back; \
         events: {:?}",
        out2.events
    );
    assert!(
        !out2.events.iter().any(|e| e.contains("dg-recovered-plain")),
        "plaintext must never appear in events; events: {:?}",
        out2.events
    );
    let recovered = out2.events.iter().any(|e| e.contains("HubRecovered"))
        || h.await_event("HubRecovered", Duration::from_secs(3)).await;
    assert!(
        recovered,
        "a successful resolution after degradation must surface HubRecovered; \
         turn events: {:?}",
        out2.events
    );
}

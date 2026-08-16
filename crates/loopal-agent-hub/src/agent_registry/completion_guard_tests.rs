use loopal_ipc::connection::Connection;
use loopal_output_guard::FinalSinkRedactionSeed;
use loopal_protocol::{AgentCompletion, QualifiedAddress};
use tokio::sync::mpsc;

use super::AgentRegistry;
use crate::types::{AgentRuntimeFacts, SpawnAuthority};

#[tokio::test]
async fn oversized_completion_is_rejected_before_cache_watch_and_parent_delivery() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let mut registry = AgentRegistry::new(event_tx);
    let (_parent_peer, parent_transport) = loopal_ipc::duplex_pair();
    let (parent, _parent_incoming) = Connection::new(parent_transport).into_listening();
    let (completion_tx, mut completion_rx) = mpsc::channel(1);
    registry
        .register_connection_with_parent("parent", parent, None, None, Some(completion_tx))
        .unwrap();
    let (_child_peer, child_transport) = loopal_ipc::duplex_pair();
    let (child, _child_incoming) = Connection::new(child_transport).into_listening();
    registry
        .register_connection_with_parent(
            "child",
            child,
            Some(QualifiedAddress::local("parent")),
            None,
            None,
        )
        .unwrap();
    let mut watcher = registry.watch_completion("child");

    let mut pending = registry.emit_agent_completion(
        "child",
        AgentCompletion::goal(Some("canary".repeat(20_000))),
    );
    let cached = registry.completion("child").unwrap();
    assert_eq!(
        cached.reason,
        loopal_output_guard::OUTPUT_GUARD_REJECTED_REASON
    );
    assert!(!cached.output().contains("canary"));
    watcher.changed().await.unwrap();
    assert!(
        !watcher
            .borrow()
            .as_ref()
            .unwrap()
            .output()
            .contains("canary")
    );
    let (_, envelope) = pending.take_parent_delivery().unwrap();
    assert!(!envelope.content.text.contains("canary"));
    assert!(
        !envelope
            .agent_completion
            .unwrap()
            .output()
            .contains("canary")
    );
    assert!(completion_rx.try_recv().is_err());
}

#[tokio::test]
async fn resolved_hub_secret_is_redacted_before_completion_cache() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let seed = FinalSinkRedactionSeed::new();
    seed.observe("token", "hub-secret".into()).unwrap();
    let mut registry = AgentRegistry::new_with_redaction_seed(event_tx, seed);

    let _pending = registry.emit_agent_completion(
        "worker",
        AgentCompletion::goal(Some("result=hub-secret".into())),
    );

    assert_eq!(
        registry.completion("worker").unwrap().output(),
        "result=<secret_ref:token>"
    );
}

#[tokio::test]
async fn workflow_execution_uses_its_trusted_completion_limit() {
    let (event_tx, _event_rx) = mpsc::channel(8);
    let mut registry = AgentRegistry::new(event_tx);
    let (_peer, transport) = loopal_ipc::duplex_pair();
    let (connection, _incoming) = Connection::new(transport).into_listening();
    registry
        .register_connection_with_parent("worker", connection, None, None, None)
        .unwrap();
    let execution = registry.current_execution("worker").unwrap();
    let limit = loopal_output_guard::MAX_AGENT_COMPLETION_RESULT_BYTES + 1;
    let mut facts = AgentRuntimeFacts::root(std::env::temp_dir(), SpawnAuthority::default());
    facts.workflow_completion_result_limit = Some(limit as u32);
    assert!(registry.set_runtime_facts(&execution, facts));

    let expected = "w".repeat(limit);
    let _pending =
        registry.emit_agent_completion("worker", AgentCompletion::goal(Some(expected.clone())));

    assert_eq!(registry.completion("worker").unwrap().output(), expected);
}

use std::sync::Arc;

use loopal_agent::registry::{AgentHandle, AgentRegistry};
use tokio_util::sync::CancellationToken;

fn make_handle(name: &str) -> AgentHandle {
    let token = CancellationToken::new();
    AgentHandle {
        id: format!("id-{name}"),
        name: name.to_string(),
        agent_type: "default".to_string(),
        cancel_token: token,
        join_handle: tokio::spawn(async {}),
        process: Arc::new(tokio::sync::Mutex::new(None)),
    }
}

#[tokio::test]
async fn test_register_and_get() {
    let mut reg = AgentRegistry::new();
    reg.register(make_handle("worker"));

    assert!(reg.get("worker").is_some());
    assert!(reg.get("nonexistent").is_none());
    assert_eq!(reg.len(), 1);
}

#[tokio::test]
async fn test_remove() {
    let mut reg = AgentRegistry::new();
    reg.register(make_handle("worker"));

    let removed = reg.remove("worker");
    assert!(removed.is_some());
    assert!(reg.is_empty());
}

#[tokio::test]
async fn test_iter() {
    let mut reg = AgentRegistry::new();
    reg.register(make_handle("a"));
    reg.register(make_handle("b"));

    let mut names: Vec<_> = reg.iter().map(|h| h.name.clone()).collect();
    names.sort();
    assert_eq!(names, vec!["a", "b"]);
}

#[tokio::test]
async fn test_shutdown_all_clears_registry() {
    let mut reg = AgentRegistry::new();
    let token_a = CancellationToken::new();
    let token_b = CancellationToken::new();
    let ta = token_a.clone();
    let tb = token_b.clone();

    reg.register(AgentHandle {
        id: "id-a".into(),
        name: "a".into(),
        agent_type: "default".into(),
        cancel_token: token_a,
        join_handle: tokio::spawn(async {}),
        process: Arc::new(tokio::sync::Mutex::new(None)),
    });
    reg.register(AgentHandle {
        id: "id-b".into(),
        name: "b".into(),
        agent_type: "default".into(),
        cancel_token: token_b,
        join_handle: tokio::spawn(async {}),
        process: Arc::new(tokio::sync::Mutex::new(None)),
    });

    assert_eq!(reg.len(), 2);
    reg.shutdown_all().await;
    assert!(reg.is_empty());
    // Tokens should be cancelled
    assert!(ta.is_cancelled());
    assert!(tb.is_cancelled());
}

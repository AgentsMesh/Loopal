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

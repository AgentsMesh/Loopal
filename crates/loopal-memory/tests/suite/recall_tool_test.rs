use std::sync::Arc;

use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_memory::{MemoryGraph, MemoryKind, MemoryNode, MemoryRecallTool};
use loopal_tool_api::{Backend, PermissionLevel, Tool, ToolContext};
use serde_json::json;

fn n(id: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind: MemoryKind::Project,
        name: format!("{} name", id),
        description: Some("desc".into()),
        file_path: format!(".loopal/memory/{}.md", id),
        body_preview: format!("body of {}", id),
        created_at: 1,
        updated_at: 1,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

fn tool_context() -> ToolContext {
    let backend: Arc<dyn Backend> = LocalBackend::new(
        std::env::temp_dir(),
        None,
        ResourceLimits::default(),
        "test-session",
    );
    ToolContext::new(backend, "test-session")
}

#[tokio::test]
async fn tool_metadata_marks_recall_as_primary() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    let t = MemoryRecallTool::new(g);
    assert_eq!(t.name(), "memory_recall");
    assert_eq!(t.permission(), PermissionLevel::ReadOnly);
    assert!(t.description().contains("THE tool"));
    assert!(t.description().to_lowercase().contains("never read"));
    assert!(t.secret_eligible_params().is_empty());
}

#[tokio::test]
async fn tool_rejects_when_no_query_or_anchor() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    let t = MemoryRecallTool::new(g);
    let result = t.execute(json!({}), &tool_context()).await.unwrap();
    assert!(result.is_error);
}

#[tokio::test]
async fn tool_returns_markdown_for_anchor_query() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    g.upsert_node(n("twitter-auto")).await.unwrap();
    let t = MemoryRecallTool::new(g);
    let r = t
        .execute(
            json!({"anchor_names": ["twitter-auto"], "depth": 0}),
            &tool_context(),
        )
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(r.content.contains("twitter-auto"));
    assert!(r.content.contains("## Direct hits"));
}

#[tokio::test]
async fn tool_returns_no_match_for_unknown_query() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    g.upsert_node(n("foo")).await.unwrap();
    let t = MemoryRecallTool::new(g);
    let r = t
        .execute(json!({"query": "definitelynonexistent"}), &tool_context())
        .await
        .unwrap();
    assert!(r.content.contains("No matching memories"));
}

#[tokio::test]
async fn tool_clamps_max_results_and_depth() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    g.upsert_node(n("foo")).await.unwrap();
    let t = MemoryRecallTool::new(g);
    let r = t
        .execute(
            json!({
                "anchor_names": ["foo"],
                "max_results": 9999,
                "depth": 99
            }),
            &tool_context(),
        )
        .await
        .unwrap();
    assert!(!r.is_error);
}

use std::sync::Arc;

use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_memory::{
    EventLogWriter, MemoryGraph, MemoryImportanceTool, MemoryKind, MemoryNode, PROJECT_MEMORY_DIR,
    fold_events,
};
use loopal_tool_api::{Backend, PermissionLevel, Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

fn node(id: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind: MemoryKind::Project,
        name: id.into(),
        description: Some("desc".into()),
        file_path: format!(".loopal/memory/{}.md", id),
        body_preview: "body".into(),
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
        "test",
    );
    ToolContext::new(backend, "test")
}

#[tokio::test]
async fn tool_metadata() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    let t = MemoryImportanceTool::new(g);
    assert_eq!(t.name(), "memory_set_importance");
    assert_eq!(t.permission(), PermissionLevel::Write);
    assert_eq!(t.secret_eligible_params(), &[] as &[&str]);
}

#[tokio::test]
async fn rejects_missing_node() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    let t = MemoryImportanceTool::new(g);
    let ctx = tool_context();
    let r = t
        .execute(json!({ "importance": 5 }), &ctx)
        .await
        .unwrap_err();
    assert!(r.to_string().contains("node"), "got: {}", r);
}

#[tokio::test]
async fn rejects_out_of_range_importance() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    let t = MemoryImportanceTool::new(g);
    let ctx = tool_context();
    let err = t
        .execute(json!({ "node": "foo", "importance": 99 }), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("importance"));
}

#[tokio::test]
async fn tag_updates_recall_stats_in_memory_and_disk() {
    let tmp_events = TempDir::new().unwrap();
    let mut graph = MemoryGraph::in_memory().unwrap();
    graph.upsert_node(node("foo")).await.unwrap();
    graph.set_event_log(Arc::new(EventLogWriter::new(
        tmp_events.path().to_path_buf(),
        "tag-test",
    )));
    let g = Arc::new(graph);

    let tool = MemoryImportanceTool::new(g.clone());
    let ctx = tool_context();
    let result = tool
        .execute(
            json!({
                "node": "foo",
                "importance": 7,
                "tags": ["critical", "incident"],
                "note": "user explicitly flagged in conversation"
            }),
            &ctx,
        )
        .await
        .unwrap();
    assert!(!result.is_error, "tool result: {:?}", result);

    let snap = g.recall_stats_snapshot("foo").expect("foo stats present");
    assert_eq!(snap.importance, 7);
    assert!(snap.importance_ts > 0);

    let folded = fold_events(tmp_events.path());
    assert_eq!(
        folded.get("foo").unwrap().importance,
        7,
        "tag must persist to event log for cross-session use"
    );
}

#[tokio::test]
async fn highest_ts_wins_on_repeated_tagging() {
    let tmp_events = TempDir::new().unwrap();
    let mut graph = MemoryGraph::in_memory().unwrap();
    graph.upsert_node(node("foo")).await.unwrap();
    graph.set_event_log(Arc::new(EventLogWriter::new(
        tmp_events.path().to_path_buf(),
        "tag-rep",
    )));
    let g = Arc::new(graph);
    let tool = MemoryImportanceTool::new(g.clone());
    let ctx = tool_context();

    tool.execute(json!({ "node": "foo", "importance": 3 }), &ctx)
        .await
        .unwrap();
    tool.execute(json!({ "node": "foo", "importance": 9 }), &ctx)
        .await
        .unwrap();

    let snap = g.recall_stats_snapshot("foo").unwrap();
    assert_eq!(snap.importance, 9, "most recent tag wins (highest ts)");
}

#[tokio::test]
async fn silently_drops_when_no_event_log_configured() {
    let g = Arc::new(MemoryGraph::in_memory().unwrap());
    let tool = MemoryImportanceTool::new(g.clone());
    let ctx = tool_context();

    let r = tool
        .execute(json!({ "node": "ghost", "importance": 5 }), &ctx)
        .await
        .unwrap();
    assert!(!r.is_error);
    assert!(
        g.recall_stats_snapshot("ghost").is_none(),
        "no log = no fold = no stats"
    );

    let _ = PROJECT_MEMORY_DIR;
}

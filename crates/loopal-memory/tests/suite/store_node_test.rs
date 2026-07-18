use loopal_memory::{MemoryGraph, MemoryKind, MemoryNode};

fn make_node(id: &str, kind: MemoryKind, path: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind,
        name: id.into(),
        description: Some(format!("desc-{}", id)),
        file_path: path.into(),
        body: format!("body of {}", id),
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
        ttl_days: Some(90),
        content_hash: "h".repeat(64),
        indexed_at: 1_700_000_000_000,
    }
}

#[tokio::test]
async fn upsert_then_get_returns_node() {
    let g = MemoryGraph::in_memory().unwrap();
    let n = make_node("foo", MemoryKind::Project, ".loopal/memory/foo.md");
    g.upsert_node(n.clone()).await.unwrap();
    let got = g.get_node("foo").await.unwrap().unwrap();
    assert_eq!(got, n);
}

#[tokio::test]
async fn get_returns_none_for_missing_id() {
    let g = MemoryGraph::in_memory().unwrap();
    assert!(g.get_node("ghost").await.unwrap().is_none());
}

#[tokio::test]
async fn upsert_is_idempotent_and_updates_fields() {
    let g = MemoryGraph::in_memory().unwrap();
    let mut n = make_node("foo", MemoryKind::Project, ".loopal/memory/foo.md");
    g.upsert_node(n.clone()).await.unwrap();
    n.description = Some("changed".into());
    n.updated_at = 1_800_000_000_000;
    n.content_hash = "i".repeat(64);
    g.upsert_node(n.clone()).await.unwrap();
    let got = g.get_node("foo").await.unwrap().unwrap();
    assert_eq!(got.description, Some("changed".into()));
    assert_eq!(got.updated_at, 1_800_000_000_000);
    assert_eq!(g.node_count().await.unwrap(), 1);
}

#[tokio::test]
async fn upsert_with_same_content_hash_preserves_updated_at() {
    let g = MemoryGraph::in_memory().unwrap();
    let mut n = make_node("foo", MemoryKind::Project, ".loopal/memory/foo.md");
    g.upsert_node(n.clone()).await.unwrap();
    n.description = Some("metadata-changed".into());
    n.updated_at = 1_800_000_000_000;
    g.upsert_node(n.clone()).await.unwrap();
    let got = g.get_node("foo").await.unwrap().unwrap();
    assert_eq!(got.description, Some("metadata-changed".into()));
    assert_eq!(got.updated_at, 1_700_000_000_000);
}

#[tokio::test]
async fn list_returns_all_ordered_by_updated_at() {
    let g = MemoryGraph::in_memory().unwrap();
    let mut a = make_node("a", MemoryKind::Project, "a.md");
    a.updated_at = 100;
    let mut b = make_node("b", MemoryKind::Feedback, "b.md");
    b.updated_at = 300;
    let mut c = make_node("c", MemoryKind::User, "c.md");
    c.updated_at = 200;
    g.upsert_node(a).await.unwrap();
    g.upsert_node(b).await.unwrap();
    g.upsert_node(c).await.unwrap();

    let nodes = g.list_nodes(None).await.unwrap();
    let ids: Vec<&str> = nodes.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, vec!["b", "c", "a"]);
}

#[tokio::test]
async fn list_by_kind_filters() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make_node("a", MemoryKind::Project, "a.md"))
        .await
        .unwrap();
    g.upsert_node(make_node("b", MemoryKind::Feedback, "b.md"))
        .await
        .unwrap();
    g.upsert_node(make_node("c", MemoryKind::Project, "c.md"))
        .await
        .unwrap();

    let projects = g.list_nodes_by_kind(MemoryKind::Project).await.unwrap();
    assert_eq!(projects.len(), 2);
    let feedback = g.list_nodes_by_kind(MemoryKind::Feedback).await.unwrap();
    assert_eq!(feedback.len(), 1);
    let users = g.list_nodes_by_kind(MemoryKind::User).await.unwrap();
    assert_eq!(users.len(), 0);
}

#[tokio::test]
async fn delete_removes_node_and_returns_true_once() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make_node("foo", MemoryKind::Project, "foo.md"))
        .await
        .unwrap();
    assert!(g.delete_node("foo").await.unwrap());
    assert!(!g.delete_node("foo").await.unwrap());
    assert!(g.get_node("foo").await.unwrap().is_none());
}

#[tokio::test]
async fn find_by_file_path_matches_unique_node() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make_node(
        "foo",
        MemoryKind::Project,
        ".loopal/memory/foo.md",
    ))
    .await
    .unwrap();
    let got = g
        .find_node_by_path(".loopal/memory/foo.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.id, "foo");
    assert!(
        g.find_node_by_path(".loopal/memory/missing.md")
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn get_many_returns_present_subset() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make_node("a", MemoryKind::Project, "a.md"))
        .await
        .unwrap();
    g.upsert_node(make_node("c", MemoryKind::Project, "c.md"))
        .await
        .unwrap();
    let got = g
        .get_nodes(&["a".into(), "b".into(), "c".into()])
        .await
        .unwrap();
    let mut ids: Vec<&str> = got.iter().map(|n| n.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["a", "c"]);
}

#[tokio::test]
async fn count_reflects_upserts_and_deletes() {
    let g = MemoryGraph::in_memory().unwrap();
    assert_eq!(g.node_count().await.unwrap(), 0);
    g.upsert_node(make_node("a", MemoryKind::Project, "a.md"))
        .await
        .unwrap();
    g.upsert_node(make_node("b", MemoryKind::Project, "b.md"))
        .await
        .unwrap();
    assert_eq!(g.node_count().await.unwrap(), 2);
    g.delete_node("a").await.unwrap();
    assert_eq!(g.node_count().await.unwrap(), 1);
}

use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn node(id: &str, path: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind: MemoryKind::Project,
        name: id.into(),
        description: Some(format!("desc-{}", id)),
        file_path: path.into(),
        body_preview: "body".into(),
        created_at: 1,
        updated_at: 1,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

fn ref_edge(src: &str, dst: &str) -> MemoryEdge {
    MemoryEdge {
        id: None,
        src_id: src.into(),
        dst_id: dst.into(),
        kind: EdgeKind::References,
        line: None,
        metadata: None,
        provenance: Provenance::Frontmatter,
        confidence: 1.0,
        created_at: 1,
    }
}

#[tokio::test]
async fn rename_preserves_incoming_edges() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("architecture", "architecture.md"))
        .await
        .unwrap();
    g.upsert_node(node("readme", "readme.md")).await.unwrap();
    g.insert_edge(ref_edge("readme", "architecture"))
        .await
        .unwrap();

    let renamed = g
        .rename_node("architecture", "system-design", "system-design.md")
        .await
        .unwrap();
    assert!(renamed);

    assert!(g.get_node("architecture").await.unwrap().is_none());
    let new_node = g.get_node("system-design").await.unwrap().unwrap();
    assert_eq!(new_node.file_path, "system-design.md");

    let incoming = g.get_incoming_edges("system-design").await.unwrap();
    assert_eq!(incoming.len(), 1);
    assert_eq!(incoming[0].src_id, "readme");
    assert_eq!(incoming[0].dst_id, "system-design");
}

#[tokio::test]
async fn rename_to_same_id_only_updates_file_path() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("foo", "foo.md")).await.unwrap();

    let renamed = g.rename_node("foo", "foo", "moved/foo.md").await.unwrap();
    assert!(renamed);

    let got = g.get_node("foo").await.unwrap().unwrap();
    assert_eq!(got.file_path, "moved/foo.md");
}

#[tokio::test]
async fn rename_refuses_when_target_id_already_exists() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a", "a.md")).await.unwrap();
    g.upsert_node(node("b", "b.md")).await.unwrap();

    let renamed = g.rename_node("a", "b", "b.md").await.unwrap();
    assert!(!renamed);

    assert!(g.get_node("a").await.unwrap().is_some());
    assert!(g.get_node("b").await.unwrap().is_some());
}

#[tokio::test]
async fn rename_missing_source_returns_false() {
    let g = MemoryGraph::in_memory().unwrap();
    let renamed = g
        .rename_node("ghost", "phantom", "phantom.md")
        .await
        .unwrap();
    assert!(!renamed);
}

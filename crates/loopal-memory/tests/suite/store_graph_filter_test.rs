use loopal_memory::{
    BfsConfig, EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance,
};

fn node(id: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind: MemoryKind::Project,
        name: id.into(),
        description: None,
        file_path: format!("{}.md", id),
        body: id.into(),
        created_at: 1,
        updated_at: 1,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

fn edge(src: &str, dst: &str, kind: EdgeKind, conf: f32) -> MemoryEdge {
    MemoryEdge {
        id: None,
        src_id: src.into(),
        dst_id: dst.into(),
        kind,
        line: None,
        metadata: None,
        provenance: Provenance::Frontmatter,
        confidence: conf,
        created_at: 1,
    }
}

#[tokio::test]
async fn bfs_min_confidence_filters_weak_edges() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a")).await.unwrap();
    g.upsert_node(node("b")).await.unwrap();
    g.upsert_node(node("c")).await.unwrap();
    g.insert_edge(edge("a", "b", EdgeKind::CoOccursSlug, 0.9))
        .await
        .unwrap();
    g.insert_edge(edge("a", "c", EdgeKind::CoOccursSlug, 0.3))
        .await
        .unwrap();

    let sub = g
        .bfs(
            &["a".into()],
            BfsConfig {
                max_depth: 1,
                min_confidence: 0.7,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut ids: Vec<&str> = sub.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["a", "b"]);
}

#[tokio::test]
async fn bfs_edge_kinds_filter() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a")).await.unwrap();
    g.upsert_node(node("b")).await.unwrap();
    g.upsert_node(node("c")).await.unwrap();
    g.insert_edge(edge("a", "b", EdgeKind::References, 1.0))
        .await
        .unwrap();
    g.insert_edge(edge("a", "c", EdgeKind::CoOccursSlug, 1.0))
        .await
        .unwrap();

    let sub = g
        .bfs(
            &["a".into()],
            BfsConfig {
                max_depth: 1,
                edge_kinds: Some(vec![EdgeKind::References]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut ids: Vec<&str> = sub.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["a", "b"]);
}

#[tokio::test]
async fn bfs_multi_edge_kinds_filter_or_semantics() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a")).await.unwrap();
    g.upsert_node(node("b")).await.unwrap();
    g.upsert_node(node("c")).await.unwrap();
    g.upsert_node(node("d")).await.unwrap();
    g.insert_edge(edge("a", "b", EdgeKind::References, 1.0))
        .await
        .unwrap();
    g.insert_edge(edge("a", "c", EdgeKind::DerivedFrom, 1.0))
        .await
        .unwrap();
    g.insert_edge(edge("a", "d", EdgeKind::CoOccursSlug, 1.0))
        .await
        .unwrap();

    let sub = g
        .bfs(
            &["a".into()],
            BfsConfig {
                max_depth: 1,
                edge_kinds: Some(vec![EdgeKind::References, EdgeKind::DerivedFrom]),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut ids: Vec<&str> = sub.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["a", "b", "c"]);
}

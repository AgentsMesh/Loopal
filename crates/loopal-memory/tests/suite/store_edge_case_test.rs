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

#[tokio::test]
async fn empty_graph_lists_and_counts_return_zero() {
    let g = MemoryGraph::in_memory().unwrap();
    assert_eq!(g.node_count().await.unwrap(), 0);
    assert_eq!(g.edge_count().await.unwrap(), 0);
    assert!(g.list_nodes(None).await.unwrap().is_empty());
    assert!(
        g.list_nodes_by_kind(MemoryKind::Project)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn bfs_on_empty_db_returns_only_unknown_start() {
    let g = MemoryGraph::in_memory().unwrap();
    let sub = g
        .bfs(&["ghost".into()], BfsConfig::default())
        .await
        .unwrap();
    assert!(sub.nodes.is_empty());
    assert!(sub.edges.is_empty());
}

#[tokio::test]
async fn bfs_with_empty_start_ids_returns_empty_subgraph() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a")).await.unwrap();
    let sub = g.bfs(&[], BfsConfig::default()).await.unwrap();
    assert!(sub.nodes.is_empty());
}

#[tokio::test]
async fn unicode_slug_is_preserved() {
    let g = MemoryGraph::in_memory().unwrap();
    let mut n = node("a");
    n.id = "中文-slug-_测试".into();
    n.name = "中文 name with emoji ✨".into();
    n.body = "测试 body 内容".into();
    n.file_path = "中文-slug-_测试.md".into();
    g.upsert_node(n.clone()).await.unwrap();
    let got = g.get_node("中文-slug-_测试").await.unwrap().unwrap();
    assert_eq!(got, n);
}

#[tokio::test]
async fn deeply_nested_paths_are_stored_verbatim() {
    let g = MemoryGraph::in_memory().unwrap();
    let mut n = node("nested");
    n.file_path = ".loopal/memory/sub/group/nested.md".into();
    g.upsert_node(n.clone()).await.unwrap();
    let got = g
        .find_node_by_path(".loopal/memory/sub/group/nested.md")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(got.file_path, n.file_path);
}

#[tokio::test]
async fn bfs_respects_limit_nodes() {
    let g = MemoryGraph::in_memory().unwrap();
    for i in 0..10 {
        g.upsert_node(node(&format!("n{}", i))).await.unwrap();
    }
    for i in 0..9 {
        let e = MemoryEdge {
            id: None,
            src_id: format!("n{}", i),
            dst_id: format!("n{}", i + 1),
            kind: EdgeKind::References,
            line: None,
            metadata: None,
            provenance: Provenance::Frontmatter,
            confidence: 1.0,
            created_at: 1,
        };
        g.insert_edge(e).await.unwrap();
    }
    let sub = g
        .bfs(
            &["n0".into()],
            BfsConfig {
                max_depth: 10,
                limit_nodes: 4,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert!(sub.nodes.len() <= 4);
}

#[tokio::test]
async fn edge_metadata_round_trip() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a")).await.unwrap();
    g.upsert_node(node("b")).await.unwrap();
    let meta = serde_json::json!({"hint": "from frontmatter", "count": 3});
    let e = MemoryEdge {
        id: None,
        src_id: "a".into(),
        dst_id: "b".into(),
        kind: EdgeKind::References,
        line: Some(42),
        metadata: Some(meta.clone()),
        provenance: Provenance::Frontmatter,
        confidence: 0.95,
        created_at: 1,
    };
    g.insert_edge(e).await.unwrap();
    let edges = g.get_outgoing_edges("a").await.unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].metadata, Some(meta));
    assert_eq!(edges[0].line, Some(42));
}

#[tokio::test]
async fn open_creates_parent_directory() {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("a").join("b").join("c").join("index.db");
    let g = MemoryGraph::open(&db_path).unwrap();
    g.upsert_node(node("a")).await.unwrap();
    assert!(db_path.exists());
}

#[tokio::test]
async fn ttl_days_optional_field_persists_none() {
    let g = MemoryGraph::in_memory().unwrap();
    let mut n = node("a");
    n.ttl_days = None;
    g.upsert_node(n.clone()).await.unwrap();
    let got = g.get_node("a").await.unwrap().unwrap();
    assert!(got.ttl_days.is_none());
}

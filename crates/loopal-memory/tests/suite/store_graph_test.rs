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
        body_preview: id.into(),
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

async fn graph_with_chain() -> MemoryGraph {
    let g = MemoryGraph::in_memory().unwrap();
    for id in ["a", "b", "c", "d"] {
        g.upsert_node(node(id)).await.unwrap();
    }
    g.insert_edge(edge("a", "b", EdgeKind::References, 1.0))
        .await
        .unwrap();
    g.insert_edge(edge("b", "c", EdgeKind::References, 1.0))
        .await
        .unwrap();
    g.insert_edge(edge("c", "d", EdgeKind::References, 1.0))
        .await
        .unwrap();
    g
}

#[tokio::test]
async fn bfs_depth_zero_returns_only_start_nodes() {
    let g = graph_with_chain().await;
    let cfg = BfsConfig {
        max_depth: 0,
        ..Default::default()
    };
    let sub = g.bfs(&["a".into()], cfg).await.unwrap();
    assert_eq!(sub.nodes.len(), 1);
    assert_eq!(sub.nodes[0].id, "a");
    assert!(sub.edges.is_empty());
}

#[tokio::test]
async fn bfs_depth_two_reaches_exactly_two_hops() {
    let g = graph_with_chain().await;
    let cfg = BfsConfig {
        max_depth: 2,
        ..Default::default()
    };
    let sub = g.bfs(&["a".into()], cfg).await.unwrap();
    let mut ids: Vec<&str> = sub.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["a", "b", "c"]);
}

#[tokio::test]
async fn bfs_is_bidirectional_symmetric() {
    let g = graph_with_chain().await;
    let cfg = BfsConfig {
        max_depth: 3,
        ..Default::default()
    };
    let from_a = g.bfs(&["a".into()], cfg).await.unwrap();
    let from_d = g
        .bfs(
            &["d".into()],
            BfsConfig {
                max_depth: 3,
                ..Default::default()
            },
        )
        .await
        .unwrap();

    let mut ids_a: Vec<String> = from_a.nodes.iter().map(|n| n.id.clone()).collect();
    let mut ids_d: Vec<String> = from_d.nodes.iter().map(|n| n.id.clone()).collect();
    ids_a.sort();
    ids_d.sort();
    assert_eq!(ids_a, vec!["a", "b", "c", "d"]);
    assert_eq!(ids_d, vec!["a", "b", "c", "d"]);
}

#[tokio::test]
async fn bfs_inbound_neighbors_reachable() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("center")).await.unwrap();
    g.upsert_node(node("inbound")).await.unwrap();
    g.upsert_node(node("outbound")).await.unwrap();
    g.insert_edge(edge("inbound", "center", EdgeKind::References, 1.0))
        .await
        .unwrap();
    g.insert_edge(edge("center", "outbound", EdgeKind::References, 1.0))
        .await
        .unwrap();

    let sub = g
        .bfs(
            &["center".into()],
            BfsConfig {
                max_depth: 1,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let mut ids: Vec<&str> = sub.nodes.iter().map(|n| n.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["center", "inbound", "outbound"]);
}

#[tokio::test]
async fn bfs_records_depth_for_each_node() {
    let g = graph_with_chain().await;
    let sub = g
        .bfs(
            &["a".into()],
            BfsConfig {
                max_depth: 3,
                ..Default::default()
            },
        )
        .await
        .unwrap();
    assert_eq!(sub.depth_by_id["a"], 0);
    assert_eq!(sub.depth_by_id["b"], 1);
    assert_eq!(sub.depth_by_id["c"], 2);
    assert_eq!(sub.depth_by_id["d"], 3);
}

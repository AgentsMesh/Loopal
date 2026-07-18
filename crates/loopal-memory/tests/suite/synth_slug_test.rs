use loopal_memory::synthesize::run_all;
use loopal_memory::synthesize::slug_cluster::group_by_prefix;
use loopal_memory::{EdgeKind, MemoryGraph, MemoryKind, MemoryNode, Provenance};

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

async fn slug_edges(g: &MemoryGraph) -> Vec<loopal_memory::MemoryEdge> {
    g.list_edges_by_provenance(Provenance::Synthesized)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::CoOccursSlug)
        .collect()
}

#[test]
fn group_by_prefix_collects_same_prefix() {
    let ids: Vec<String> = vec![
        "twitter-a".into(),
        "twitter-b".into(),
        "twitter-c".into(),
        "scanner-x".into(),
        "scanner-y".into(),
        "lonely".into(),
    ];
    let map = group_by_prefix(&ids);
    assert_eq!(map.get("twitter").map(|v| v.len()), Some(3));
    assert_eq!(map.get("scanner").map(|v| v.len()), Some(2));
    assert!(!map.contains_key("lonely"));
}

#[test]
fn group_by_prefix_ignores_no_dash_ids() {
    let ids: Vec<String> = vec!["foo".into(), "bar".into(), "baz".into()];
    let map = group_by_prefix(&ids);
    assert!(map.is_empty());
}

#[tokio::test]
async fn synthesize_creates_pair_edges_within_cluster() {
    let g = MemoryGraph::in_memory().unwrap();
    for id in ["twitter-a", "twitter-b", "twitter-c"] {
        g.upsert_node(node(id)).await.unwrap();
    }
    run_all(&g).await.unwrap();
    let edges = slug_edges(&g).await;
    assert_eq!(edges.len(), 3);
    let expected_conf = 0.5_f32;
    assert!((edges[0].confidence - expected_conf).abs() < 1e-6);
}

#[tokio::test]
async fn synthesize_confidence_inverse_to_cluster_size() {
    let g = MemoryGraph::in_memory().unwrap();
    for id in ["x-1", "x-2", "x-3", "x-4", "x-5"] {
        g.upsert_node(node(id)).await.unwrap();
    }
    run_all(&g).await.unwrap();
    let edges = slug_edges(&g).await;
    assert_eq!(edges.len(), 10);
    // N=5 大簇：1/(N-1) = 0.25 < 0.3 默认 BFS 阈值 → query-mode 自动剪枝；
    // 但 slug_cluster 仍持久化这条边，anchor-mode (min_confidence=0.1) 可以达到。
    let expected = 1.0_f32 / 4.0;
    assert!((edges[0].confidence - expected).abs() < 1e-6);
}

#[tokio::test]
async fn synthesize_does_nothing_when_no_clusters_exist() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("twitter-only")).await.unwrap();
    g.upsert_node(node("scanner-only")).await.unwrap();
    run_all(&g).await.unwrap();
    assert!(slug_edges(&g).await.is_empty());
}

#[tokio::test]
async fn synthesize_is_idempotent() {
    let g = MemoryGraph::in_memory().unwrap();
    for id in ["a-1", "a-2", "a-3"] {
        g.upsert_node(node(id)).await.unwrap();
    }
    run_all(&g).await.unwrap();
    run_all(&g).await.unwrap();
    let edges = slug_edges(&g).await;
    assert_eq!(edges.len(), 3);
}

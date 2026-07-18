use loopal_memory::graph::recall::{RecallParams, recall};
use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn make(id: &str, kind: MemoryKind, name: &str, body: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind,
        name: name.into(),
        description: Some(format!("{}: described", name)),
        file_path: format!("{}.md", id),
        body: body.into(),
        created_at: 1,
        updated_at: 1,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

#[tokio::test]
async fn query_uses_fts5_to_seed_anchors() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make(
        "rate-limit",
        MemoryKind::Project,
        "Twitter rate limit policy",
        "exceed 50 calls then cool down",
    ))
    .await
    .unwrap();
    g.upsert_node(make(
        "scanner",
        MemoryKind::Project,
        "Scanner state",
        "track ids of processed items",
    ))
    .await
    .unwrap();

    let params = RecallParams {
        query: Some("rate".into()),
        depth: 0,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    assert_eq!(r.direct_hits.len(), 1);
    assert_eq!(r.direct_hits[0].node.id, "rate-limit");
}

#[tokio::test]
async fn query_with_depth_expands_to_neighbors() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make(
        "a",
        MemoryKind::Project,
        "uniquequeryterm a",
        "anchor body",
    ))
    .await
    .unwrap();
    g.upsert_node(make("b", MemoryKind::Project, "neighbor", "neighbor body"))
        .await
        .unwrap();
    g.insert_edge(MemoryEdge {
        id: None,
        src_id: "a".into(),
        dst_id: "b".into(),
        kind: EdgeKind::References,
        line: None,
        metadata: None,
        provenance: Provenance::Frontmatter,
        confidence: 1.0,
        created_at: 1,
    })
    .await
    .unwrap();

    let params = RecallParams {
        query: Some("uniquequeryterm".into()),
        depth: 1,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    assert_eq!(r.direct_hits.len(), 1);
    let ids: Vec<&str> = r.neighbors.iter().map(|n| n.node.id.as_str()).collect();
    assert_eq!(ids, vec!["b"]);
}

#[tokio::test]
async fn query_with_no_match_returns_empty_result() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make("a", MemoryKind::Project, "alpha", "alpha body"))
        .await
        .unwrap();
    let params = RecallParams {
        query: Some("nothinghere".into()),
        depth: 2,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    assert!(r.direct_hits.is_empty());
    assert!(r.neighbors.is_empty());
}

#[tokio::test]
async fn co_occurring_returned_when_include_synthesized() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make("a", MemoryKind::Project, "anchor uniquefts", "anchor"))
        .await
        .unwrap();
    g.upsert_node(make("b", MemoryKind::Project, "co", "co body"))
        .await
        .unwrap();
    g.insert_edge(MemoryEdge {
        id: None,
        src_id: "a".into(),
        dst_id: "b".into(),
        kind: EdgeKind::CoOccursSlug,
        line: None,
        metadata: None,
        provenance: Provenance::Synthesized,
        confidence: 0.85,
        created_at: 1,
    })
    .await
    .unwrap();

    let params = RecallParams {
        query: Some("uniquefts".into()),
        depth: 0,
        include_synthesized: true,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    assert_eq!(r.co_occurring.len(), 1);
    assert_eq!(r.co_occurring[0].node.id, "b");
}

#[tokio::test]
async fn co_occurring_suppressed_when_excluded() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(make(
        "a",
        MemoryKind::Project,
        "anchor uniquefts2",
        "anchor",
    ))
    .await
    .unwrap();
    g.upsert_node(make("b", MemoryKind::Project, "co", "co"))
        .await
        .unwrap();
    g.insert_edge(MemoryEdge {
        id: None,
        src_id: "a".into(),
        dst_id: "b".into(),
        kind: EdgeKind::CoOccursSlug,
        line: None,
        metadata: None,
        provenance: Provenance::Synthesized,
        confidence: 0.85,
        created_at: 1,
    })
    .await
    .unwrap();

    let params = RecallParams {
        query: Some("uniquefts2".into()),
        depth: 0,
        include_synthesized: false,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    assert!(r.co_occurring.is_empty());
}

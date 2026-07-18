use loopal_memory::graph::recall;
use loopal_memory::graph::recall::RecallParams;
use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn node(id: &str, kind: MemoryKind) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind,
        name: id.into(),
        description: Some(format!("desc-{}", id)),
        file_path: format!("{}.md", id),
        body: format!("body of {}", id),
        created_at: 1,
        updated_at: 1,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

fn edge(src: &str, dst: &str, kind: EdgeKind, conf: f32, prov: Provenance) -> MemoryEdge {
    MemoryEdge {
        id: None,
        src_id: src.into(),
        dst_id: dst.into(),
        kind,
        line: None,
        metadata: None,
        provenance: prov,
        confidence: conf,
        created_at: 1,
    }
}

async fn seed_graph() -> MemoryGraph {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("twitter-auto", MemoryKind::Project))
        .await
        .unwrap();
    g.upsert_node(node("twitter-long", MemoryKind::Project))
        .await
        .unwrap();
    g.upsert_node(node("chrome-cdp", MemoryKind::Project))
        .await
        .unwrap();
    g.upsert_node(node("user-style", MemoryKind::User))
        .await
        .unwrap();
    g.insert_edge(edge(
        "twitter-auto",
        "twitter-long",
        EdgeKind::References,
        1.0,
        Provenance::Frontmatter,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "twitter-auto",
        "chrome-cdp",
        EdgeKind::References,
        1.0,
        Provenance::InlineLink,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "twitter-auto",
        "user-style",
        EdgeKind::DerivedFrom,
        1.0,
        Provenance::Synthesized,
    ))
    .await
    .unwrap();
    g
}

#[tokio::test]
async fn anchor_names_resolve_to_direct_hits() {
    let g = seed_graph().await;
    let params = RecallParams {
        anchor_names: vec!["twitter-auto".into()],
        depth: 0,
        ..Default::default()
    };
    let r = recall::recall(&g, &params).await.unwrap();
    assert_eq!(r.direct_hits.len(), 1);
    assert_eq!(r.direct_hits[0].node.id, "twitter-auto");
    assert!(r.neighbors.is_empty());
}

#[tokio::test]
async fn anchor_with_depth_one_brings_neighbors() {
    let g = seed_graph().await;
    let params = RecallParams {
        anchor_names: vec!["twitter-auto".into()],
        depth: 1,
        ..Default::default()
    };
    let r = recall::recall(&g, &params).await.unwrap();
    let mut ids: Vec<&str> = r.neighbors.iter().map(|n| n.node.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["chrome-cdp", "twitter-long", "user-style"]);
}

#[tokio::test]
async fn anchor_inbound_neighbor_visible() {
    let g = seed_graph().await;
    let params = RecallParams {
        anchor_names: vec!["twitter-long".into()],
        depth: 1,
        ..Default::default()
    };
    let r = recall::recall(&g, &params).await.unwrap();
    let ids: Vec<&str> = r.neighbors.iter().map(|n| n.node.id.as_str()).collect();
    assert!(ids.contains(&"twitter-auto"));
}

#[tokio::test]
async fn unknown_anchor_returns_empty() {
    let g = seed_graph().await;
    let params = RecallParams {
        anchor_names: vec!["ghost".into()],
        depth: 1,
        ..Default::default()
    };
    let r = recall::recall(&g, &params).await.unwrap();
    assert!(r.direct_hits.is_empty());
}

#[tokio::test]
async fn no_query_no_anchor_yields_empty() {
    let g = seed_graph().await;
    let r = recall::recall(&g, &RecallParams::default()).await.unwrap();
    assert!(r.direct_hits.is_empty());
    assert!(r.neighbors.is_empty());
}

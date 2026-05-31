use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

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

fn edge(src: &str, dst: &str, kind: EdgeKind, prov: Provenance, conf: f32) -> MemoryEdge {
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

async fn setup() -> MemoryGraph {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a")).await.unwrap();
    g.upsert_node(node("b")).await.unwrap();
    g.upsert_node(node("c")).await.unwrap();
    g
}

#[tokio::test]
async fn insert_returns_rowid_and_increments_count() {
    let g = setup().await;
    let e = edge("a", "b", EdgeKind::References, Provenance::Frontmatter, 1.0);
    let id = g.insert_edge(e).await.unwrap();
    assert!(id > 0);
    assert_eq!(g.edge_count().await.unwrap(), 1);
}

#[tokio::test]
async fn outgoing_and_incoming_are_separate() {
    let g = setup().await;
    g.insert_edge(edge(
        "a",
        "b",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "a",
        "c",
        EdgeKind::References,
        Provenance::InlineLink,
        1.0,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "c",
        "a",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();

    let out_a = g.get_outgoing_edges("a").await.unwrap();
    assert_eq!(out_a.len(), 2);
    let in_a = g.get_incoming_edges("a").await.unwrap();
    assert_eq!(in_a.len(), 1);
    assert_eq!(in_a[0].src_id, "c");
}

#[tokio::test]
async fn duplicate_triple_updates_instead_of_inserting() {
    let g = setup().await;
    let mut e = edge("a", "b", EdgeKind::References, Provenance::Frontmatter, 0.5);
    g.insert_edge(e.clone()).await.unwrap();
    e.confidence = 0.9;
    g.insert_edge(e).await.unwrap();
    let edges = g.get_outgoing_edges("a").await.unwrap();
    assert_eq!(edges.len(), 1);
    assert!((edges[0].confidence - 0.9).abs() < 1e-6);
}

#[tokio::test]
async fn different_provenance_creates_separate_edges() {
    let g = setup().await;
    g.insert_edge(edge(
        "a",
        "b",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "a",
        "b",
        EdgeKind::References,
        Provenance::InlineLink,
        1.0,
    ))
    .await
    .unwrap();
    let edges = g.get_outgoing_edges("a").await.unwrap();
    assert_eq!(edges.len(), 2);
}

#[tokio::test]
async fn count_incoming_returns_correct_value() {
    let g = setup().await;
    g.insert_edge(edge(
        "a",
        "c",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "b",
        "c",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    assert_eq!(g.count_incoming("c").await.unwrap(), 2);
    assert_eq!(g.count_incoming("a").await.unwrap(), 0);
}

#[tokio::test]
async fn delete_node_cascades_to_edges() {
    let g = setup().await;
    g.insert_edge(edge(
        "a",
        "b",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "b",
        "c",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    assert_eq!(g.edge_count().await.unwrap(), 2);

    g.delete_node("b").await.unwrap();
    assert_eq!(g.edge_count().await.unwrap(), 0);
}

#[tokio::test]
async fn delete_edges_for_node_removes_both_directions() {
    let g = setup().await;
    g.insert_edge(edge(
        "a",
        "b",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "c",
        "b",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    let n = g.delete_edges_for_node("b").await.unwrap();
    assert_eq!(n, 2);
    assert_eq!(g.edge_count().await.unwrap(), 0);
}

#[tokio::test]
async fn list_by_provenance_filters_correctly() {
    let g = setup().await;
    g.insert_edge(edge(
        "a",
        "b",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "a",
        "c",
        EdgeKind::CoOccursSlug,
        Provenance::Synthesized,
        0.8,
    ))
    .await
    .unwrap();
    let synth = g
        .list_edges_by_provenance(Provenance::Synthesized)
        .await
        .unwrap();
    assert_eq!(synth.len(), 1);
    assert_eq!(synth[0].kind, EdgeKind::CoOccursSlug);
}

#[tokio::test]
async fn delete_edges_by_provenance_targets_correctly() {
    let g = setup().await;
    g.insert_edge(edge(
        "a",
        "b",
        EdgeKind::References,
        Provenance::Frontmatter,
        1.0,
    ))
    .await
    .unwrap();
    g.insert_edge(edge(
        "a",
        "c",
        EdgeKind::CoOccursSlug,
        Provenance::Synthesized,
        0.8,
    ))
    .await
    .unwrap();
    let n = g
        .delete_edges_by_provenance(Provenance::Synthesized)
        .await
        .unwrap();
    assert_eq!(n, 1);
    assert_eq!(g.edge_count().await.unwrap(), 1);
}

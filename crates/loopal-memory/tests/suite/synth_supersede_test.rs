use loopal_memory::synthesize::run_all;
use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn node(id: &str, body: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind: MemoryKind::Project,
        name: id.into(),
        description: None,
        file_path: format!("{}.md", id),
        body: body.into(),
        created_at: 1,
        updated_at: 1,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

async fn superseded_edges(g: &MemoryGraph) -> Vec<MemoryEdge> {
    g.list_edges_by_provenance(Provenance::Synthesized)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::SupersededBy)
        .collect()
}

#[tokio::test]
async fn supersedes_keyword_creates_edge() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("old-policy", "old text")).await.unwrap();
    g.upsert_node(node(
        "new-policy",
        "this supersedes [[old-policy]] with refined rules",
    ))
    .await
    .unwrap();

    run_all(&g).await.unwrap();
    let edges = superseded_edges(&g).await;
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].src_id, "old-policy");
    assert_eq!(edges[0].dst_id, "new-policy");
}

#[tokio::test]
async fn chinese_keyword_works() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("old", "旧的")).await.unwrap();
    g.upsert_node(node("new", "本条取代 [[old]]"))
        .await
        .unwrap();

    run_all(&g).await.unwrap();
    assert_eq!(superseded_edges(&g).await.len(), 1);
}

#[tokio::test]
async fn replaces_keyword_also_recognized() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a", "")).await.unwrap();
    g.upsert_node(node("b", "this replaces [[a]] completely"))
        .await
        .unwrap();

    run_all(&g).await.unwrap();
    assert_eq!(superseded_edges(&g).await.len(), 1);
}

#[tokio::test]
async fn no_keyword_yields_nothing() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a", "nothing special here"))
        .await
        .unwrap();
    g.upsert_node(node("b", "just references [[a]] normally"))
        .await
        .unwrap();

    run_all(&g).await.unwrap();
    assert!(superseded_edges(&g).await.is_empty());
}

#[tokio::test]
async fn self_reference_skipped() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a", "this supersedes [[a]]"))
        .await
        .unwrap();
    run_all(&g).await.unwrap();
    assert!(superseded_edges(&g).await.is_empty());
}

use loopal_memory::synthesize::run_all;
use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn node(id: &str, kind: MemoryKind) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind,
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

fn references_edge(src: &str, dst: &str) -> MemoryEdge {
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

async fn derived_edges(g: &MemoryGraph) -> Vec<MemoryEdge> {
    g.list_edges_by_provenance(Provenance::Synthesized)
        .await
        .unwrap()
        .into_iter()
        .filter(|e| e.kind == EdgeKind::DerivedFrom)
        .collect()
}

#[tokio::test]
async fn project_referencing_user_yields_derived_from() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("policy", MemoryKind::Project))
        .await
        .unwrap();
    g.upsert_node(node("style-pref", MemoryKind::User))
        .await
        .unwrap();
    g.insert_edge(references_edge("policy", "style-pref"))
        .await
        .unwrap();

    run_all(&g).await.unwrap();

    let synth = derived_edges(&g).await;
    assert_eq!(synth.len(), 1);
    assert_eq!(synth[0].src_id, "policy");
    assert_eq!(synth[0].dst_id, "style-pref");
}

#[tokio::test]
async fn project_referencing_non_user_yields_nothing() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a", MemoryKind::Project)).await.unwrap();
    g.upsert_node(node("b", MemoryKind::Project)).await.unwrap();
    g.upsert_node(node("c", MemoryKind::Reference))
        .await
        .unwrap();
    g.insert_edge(references_edge("a", "b")).await.unwrap();
    g.insert_edge(references_edge("a", "c")).await.unwrap();

    run_all(&g).await.unwrap();

    assert!(derived_edges(&g).await.is_empty());
}

#[tokio::test]
async fn non_project_source_ignored_even_with_user_target() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("ref", MemoryKind::Reference))
        .await
        .unwrap();
    g.upsert_node(node("user-pref", MemoryKind::User))
        .await
        .unwrap();
    g.insert_edge(references_edge("ref", "user-pref"))
        .await
        .unwrap();

    run_all(&g).await.unwrap();

    assert!(derived_edges(&g).await.is_empty());
}

#[tokio::test]
async fn is_idempotent_under_repeated_runs() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("p", MemoryKind::Project)).await.unwrap();
    g.upsert_node(node("u", MemoryKind::User)).await.unwrap();
    g.insert_edge(references_edge("p", "u")).await.unwrap();

    run_all(&g).await.unwrap();
    run_all(&g).await.unwrap();
    let synth = derived_edges(&g).await;
    assert_eq!(synth.len(), 1);
}

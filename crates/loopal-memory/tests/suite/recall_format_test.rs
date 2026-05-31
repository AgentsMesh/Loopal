use loopal_memory::graph::format::format_recall;
use loopal_memory::graph::recall::{RecallParams, recall};
use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn n(id: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind: MemoryKind::Project,
        name: format!("{} name", id),
        description: Some(format!("{} description", id)),
        file_path: format!(".loopal/memory/{}.md", id),
        body_preview: format!("{}: body preview text", id),
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
        ttl_days: Some(90),
        content_hash: "h".repeat(64),
        indexed_at: 1_700_000_000_000,
    }
}

#[tokio::test]
async fn empty_result_yields_no_match_message() {
    let g = MemoryGraph::in_memory().unwrap();
    let r = recall(&g, &RecallParams::default()).await.unwrap();
    let s = format_recall(&r);
    assert!(s.contains("No matching memories found"));
}

#[tokio::test]
async fn direct_hits_appear_under_heading() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(n("twitter-auto")).await.unwrap();
    let params = RecallParams {
        anchor_names: vec!["twitter-auto".into()],
        depth: 0,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    let s = format_recall(&r);
    assert!(s.contains("## Direct hits"));
    assert!(s.contains("twitter-auto"));
    assert!(s.contains("body preview"));
    assert!(s.contains(".loopal/memory/twitter-auto.md"));
}

#[tokio::test]
async fn trail_section_includes_edges_with_provenance() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(n("a")).await.unwrap();
    g.upsert_node(n("b")).await.unwrap();
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
        anchor_names: vec!["a".into()],
        depth: 2,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    let s = format_recall(&r);
    assert!(s.contains("## Trail"));
    assert!(s.contains("references"));
}

#[tokio::test]
async fn anti_read_guidance_appended_for_non_empty_results() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(n("a")).await.unwrap();
    let params = RecallParams {
        anchor_names: vec!["a".into()],
        depth: 0,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    let s = format_recall(&r);
    assert!(s.contains("memory_recall"));
    assert!(s.contains("Avoid Read"));
}

#[tokio::test]
async fn synth_edges_marked_in_trail() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(n("a")).await.unwrap();
    g.upsert_node(n("b")).await.unwrap();
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
        anchor_names: vec!["a".into()],
        depth: 1,
        min_confidence: 0.5,
        ..Default::default()
    };
    let r = recall(&g, &params).await.unwrap();
    let s = format_recall(&r);
    assert!(s.contains("synth"));
    assert!(s.contains("co_occurs"));
}

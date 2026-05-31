use loopal_memory::render::render_memory_md;
use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn node(id: &str, kind: MemoryKind, desc: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind,
        name: id.into(),
        description: Some(desc.into()),
        file_path: format!(".loopal/memory/{}.md", id),
        body_preview: format!("body of {}", id),
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1_700_000_000_000,
    }
}

#[tokio::test]
async fn empty_graph_yields_minimal_index() {
    let g = MemoryGraph::in_memory().unwrap();
    let md = render_memory_md(&g).await.unwrap();
    assert!(md.contains("# Memory Index"));
    assert!(md.contains("node_count: 0"));
    assert!(md.contains("Use memory_recall"));
}

#[tokio::test]
async fn renders_section_per_kind() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("proj-a", MemoryKind::Project, "project a"))
        .await
        .unwrap();
    g.upsert_node(node("fb-1", MemoryKind::Feedback, "feedback"))
        .await
        .unwrap();
    g.upsert_node(node("user-x", MemoryKind::User, "user pref"))
        .await
        .unwrap();
    g.upsert_node(node("ref-y", MemoryKind::Reference, "reference"))
        .await
        .unwrap();

    let md = render_memory_md(&g).await.unwrap();
    assert!(md.contains("## Project"));
    assert!(md.contains("## Feedback"));
    assert!(md.contains("## User"));
    assert!(md.contains("## Reference"));
    assert!(md.contains("[[proj-a]]"));
}

#[tokio::test]
async fn entry_includes_inbound_count_when_referenced() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("popular", MemoryKind::Project, "p"))
        .await
        .unwrap();
    g.upsert_node(node("a", MemoryKind::Project, "a"))
        .await
        .unwrap();
    g.upsert_node(node("b", MemoryKind::Project, "b"))
        .await
        .unwrap();
    for src in ["a", "b"] {
        g.insert_edge(MemoryEdge {
            id: None,
            src_id: src.into(),
            dst_id: "popular".into(),
            kind: EdgeKind::References,
            line: None,
            metadata: None,
            provenance: Provenance::Frontmatter,
            confidence: 1.0,
            created_at: 1,
        })
        .await
        .unwrap();
    }
    let md = render_memory_md(&g).await.unwrap();
    assert!(md.contains("[[popular]] (referenced by 2)"));
}

#[tokio::test]
async fn index_kind_node_is_excluded_from_sections() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("MEMORY", MemoryKind::Index, "the index"))
        .await
        .unwrap();
    g.upsert_node(node("p", MemoryKind::Project, "p"))
        .await
        .unwrap();
    let md = render_memory_md(&g).await.unwrap();
    assert!(!md.contains("[[MEMORY]]"));
    assert!(md.contains("[[p]]"));
}

#[tokio::test]
async fn anti_read_guidance_present() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("p", MemoryKind::Project, "p"))
        .await
        .unwrap();
    let md = render_memory_md(&g).await.unwrap();
    assert!(md.contains("Do NOT Read individual .md files"));
    assert!(md.contains("memory_recall"));
}

#[tokio::test]
async fn frontmatter_has_generated_from_graph_marker() {
    let g = MemoryGraph::in_memory().unwrap();
    let md = render_memory_md(&g).await.unwrap();
    assert!(md.starts_with("---"));
    assert!(md.contains("generated_from: graph"));
}

#[tokio::test]
async fn recent_section_orders_by_updated_at_desc() {
    let g = MemoryGraph::in_memory().unwrap();
    let mut a = node("old", MemoryKind::Project, "old node");
    a.updated_at = 1_700_000_000_000;
    let mut b = node("new", MemoryKind::Project, "new node");
    b.updated_at = 1_900_000_000_000;
    g.upsert_node(a).await.unwrap();
    g.upsert_node(b).await.unwrap();
    let md = render_memory_md(&g).await.unwrap();
    let recent_section = md.split("## Recently added").nth(1).unwrap_or("");
    let new_pos = recent_section.find("[[new]]").unwrap_or(usize::MAX);
    let old_pos = recent_section.find("[[old]]").unwrap_or(usize::MAX);
    assert!(new_pos < old_pos);
}

use loopal_memory::{EdgeKind, MemoryEdge, MemoryGraph, MemoryKind, MemoryNode, Provenance};

fn node(id: &str, ttl_days: Option<u32>, created_at: i64) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind: MemoryKind::Project,
        name: id.into(),
        description: None,
        file_path: format!("{}.md", id),
        body_preview: id.into(),
        created_at,
        updated_at: created_at,
        ttl_days,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

#[tokio::test]
async fn high_impact_orders_by_inbound_count() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("hub", None, 1)).await.unwrap();
    g.upsert_node(node("solo", None, 1)).await.unwrap();
    for s in ["a", "b", "c"] {
        g.upsert_node(node(s, None, 1)).await.unwrap();
        g.insert_edge(MemoryEdge {
            id: None,
            src_id: s.into(),
            dst_id: "hub".into(),
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

    let hi = g.find_high_impact(10).await.unwrap();
    assert_eq!(hi[0].0.id, "hub");
    assert_eq!(hi[0].1, 3);
}

#[tokio::test]
async fn high_impact_excludes_synthesized_edges() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("hub", None, 1)).await.unwrap();
    g.upsert_node(node("a", None, 1)).await.unwrap();
    g.insert_edge(MemoryEdge {
        id: None,
        src_id: "a".into(),
        dst_id: "hub".into(),
        kind: EdgeKind::CoOccursSlug,
        line: None,
        metadata: None,
        provenance: Provenance::Synthesized,
        confidence: 1.0,
        created_at: 1,
    })
    .await
    .unwrap();

    let hi = g.find_high_impact(10).await.unwrap();
    assert!(hi.is_empty());
}

#[tokio::test]
async fn expired_returns_only_past_ttl() {
    let g = MemoryGraph::in_memory().unwrap();
    let now = chrono::Utc::now().timestamp_millis();
    let day_ms = 1000 * 60 * 60 * 24;
    g.upsert_node(node("fresh", Some(90), now)).await.unwrap();
    g.upsert_node(node("expired", Some(90), now - 200 * day_ms))
        .await
        .unwrap();
    g.upsert_node(node("forever", None, now - 1000 * day_ms))
        .await
        .unwrap();

    let exp = g.find_expired().await.unwrap();
    let ids: Vec<&str> = exp.iter().map(|n| n.id.as_str()).collect();
    assert_eq!(ids, vec!["expired"]);
}

#[tokio::test]
async fn conflicting_pairs_returns_contradicts_edges() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node("a", None, 1)).await.unwrap();
    g.upsert_node(node("b", None, 1)).await.unwrap();
    g.upsert_node(node("c", None, 1)).await.unwrap();
    g.insert_edge(MemoryEdge {
        id: None,
        src_id: "a".into(),
        dst_id: "b".into(),
        kind: EdgeKind::Contradicts,
        line: None,
        metadata: None,
        provenance: Provenance::UserStated,
        confidence: 1.0,
        created_at: 1,
    })
    .await
    .unwrap();
    g.insert_edge(MemoryEdge {
        id: None,
        src_id: "a".into(),
        dst_id: "c".into(),
        kind: EdgeKind::References,
        line: None,
        metadata: None,
        provenance: Provenance::Frontmatter,
        confidence: 1.0,
        created_at: 1,
    })
    .await
    .unwrap();

    let pairs = g.find_conflicting_pairs().await.unwrap();
    assert_eq!(pairs.len(), 1);
    assert_eq!(pairs[0], ("a".to_string(), "b".to_string()));
}

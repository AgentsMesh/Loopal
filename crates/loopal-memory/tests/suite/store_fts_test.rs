use loopal_memory::{MemoryGraph, MemoryKind, MemoryNode};

fn node_with(id: &str, kind: MemoryKind, name: &str, desc: &str, body: &str) -> MemoryNode {
    MemoryNode {
        id: id.into(),
        kind,
        name: name.into(),
        description: Some(desc.into()),
        file_path: format!("{}.md", id),
        body_preview: body.into(),
        created_at: 1,
        updated_at: 1,
        ttl_days: None,
        content_hash: "h".repeat(64),
        indexed_at: 1,
    }
}

#[tokio::test]
async fn search_finds_node_by_body_keyword() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node_with(
        "rate-limit",
        MemoryKind::Project,
        "Twitter rate limits",
        "rate limiting rules",
        "do not exceed 50 calls per hour",
    ))
    .await
    .unwrap();
    g.upsert_node(node_with(
        "scanner",
        MemoryKind::Project,
        "Scanner idempotency",
        "state machine for scanner",
        "track processed ids",
    ))
    .await
    .unwrap();

    let hits = g.search("rate", None, 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].node.id, "rate-limit");
}

#[tokio::test]
async fn search_finds_node_by_name_or_description() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node_with(
        "foo",
        MemoryKind::Project,
        "uniquetwitter",
        "describes a thing",
        "body has no special words",
    ))
    .await
    .unwrap();

    let hits = g.search("uniquetwitter", None, 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    let hits = g.search("describes", None, 10).await.unwrap();
    assert_eq!(hits.len(), 1);
}

#[tokio::test]
async fn search_with_kind_filter_excludes_other_kinds() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node_with(
        "p",
        MemoryKind::Project,
        "rate limit project",
        "",
        "twitter",
    ))
    .await
    .unwrap();
    g.upsert_node(node_with(
        "f",
        MemoryKind::Feedback,
        "rate limit feedback",
        "",
        "twitter",
    ))
    .await
    .unwrap();

    let hits = g
        .search("twitter", Some(MemoryKind::Feedback), 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].node.id, "f");
}

#[tokio::test]
async fn search_respects_limit() {
    let g = MemoryGraph::in_memory().unwrap();
    for i in 0..5 {
        g.upsert_node(node_with(
            &format!("n{}", i),
            MemoryKind::Project,
            &format!("rate node {}", i),
            "",
            "twitter rate",
        ))
        .await
        .unwrap();
    }
    let hits = g.search("rate", None, 3).await.unwrap();
    assert_eq!(hits.len(), 3);
}

#[tokio::test]
async fn search_returns_empty_for_no_match() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node_with(
        "foo",
        MemoryKind::Project,
        "hello world",
        "",
        "body",
    ))
    .await
    .unwrap();
    let hits = g.search("nonexistentterm", None, 10).await.unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn search_returns_empty_for_empty_query() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node_with("foo", MemoryKind::Project, "twitter", "", "body"))
        .await
        .unwrap();
    let hits = g.search("", None, 10).await.unwrap();
    assert!(hits.is_empty());
    let hits = g.search("   ", None, 10).await.unwrap();
    assert!(hits.is_empty());
}

#[tokio::test]
async fn deleted_node_no_longer_appears_in_fts() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node_with(
        "foo",
        MemoryKind::Project,
        "uniqueterm",
        "",
        "body",
    ))
    .await
    .unwrap();
    assert_eq!(g.search("uniqueterm", None, 10).await.unwrap().len(), 1);
    g.delete_node("foo").await.unwrap();
    assert_eq!(g.search("uniqueterm", None, 10).await.unwrap().len(), 0);
}

#[tokio::test]
async fn updated_node_reflects_new_body_in_fts() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_node(node_with(
        "foo",
        MemoryKind::Project,
        "original",
        "",
        "alpha",
    ))
    .await
    .unwrap();
    assert_eq!(g.search("alpha", None, 10).await.unwrap().len(), 1);

    let mut n = node_with("foo", MemoryKind::Project, "updated", "", "beta");
    n.updated_at = 999;
    g.upsert_node(n).await.unwrap();

    assert_eq!(g.search("alpha", None, 10).await.unwrap().len(), 0);
    assert_eq!(g.search("beta", None, 10).await.unwrap().len(), 1);
}

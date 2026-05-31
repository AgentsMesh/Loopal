use loopal_memory::MemoryGraph;

#[tokio::test]
async fn file_cache_missing_returns_none() {
    let g = MemoryGraph::in_memory().unwrap();
    let result = g.get_file_cache("nonexistent.md").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn file_cache_round_trip() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_file_cache(
        "foo.md",
        "abc123",
        1024,
        1_700_000_000_000,
        1_700_000_001_000,
    )
    .await
    .unwrap();

    let entry = g.get_file_cache("foo.md").await.unwrap().unwrap();
    assert_eq!(entry.size, 1024);
    assert_eq!(entry.modified_at, 1_700_000_000_000);
}

#[tokio::test]
async fn file_cache_upsert_overwrites() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_file_cache("foo.md", "v1", 100, 1, 2)
        .await
        .unwrap();
    g.upsert_file_cache("foo.md", "v2", 200, 3, 4)
        .await
        .unwrap();

    let entry = g.get_file_cache("foo.md").await.unwrap().unwrap();
    assert_eq!(entry.size, 200);
    assert_eq!(entry.modified_at, 3);
}

#[tokio::test]
async fn file_cache_delete_removes_entry() {
    let g = MemoryGraph::in_memory().unwrap();
    g.upsert_file_cache("foo.md", "abc", 1, 1, 1).await.unwrap();
    let removed = g.delete_file_cache("foo.md").await.unwrap();
    assert!(removed);
    assert!(g.get_file_cache("foo.md").await.unwrap().is_none());
}

#[tokio::test]
async fn file_cache_delete_missing_returns_false() {
    let g = MemoryGraph::in_memory().unwrap();
    let removed = g.delete_file_cache("nonexistent.md").await.unwrap();
    assert!(!removed);
}

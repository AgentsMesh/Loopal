use loopal_memory::MemoryGraph;
use loopal_memory::sync::scan_directory;

#[tokio::test]
async fn fresh_directory_initializes_full_pipeline() {
    let temp = tempfile::tempdir().unwrap();
    let memory_dir = temp.path().join(".loopal/memory");
    std::fs::create_dir_all(&memory_dir).unwrap();

    std::fs::write(
        memory_dir.join("twitter-a.md"),
        "---\nname: twitter-a\ntype: project\n---\nrate limit and cooldown\n",
    )
    .unwrap();
    std::fs::write(
        memory_dir.join("twitter-b.md"),
        "---\nname: twitter-b\ntype: project\n---\ncooldown rate enforcement\n",
    )
    .unwrap();

    let db_path = memory_dir.join(".index.db");
    let g = MemoryGraph::open(&db_path).unwrap();
    let stats = scan_directory(&g, &memory_dir).await.unwrap();

    assert_eq!(stats.files_scanned, 2);
    assert!(stats.errors.is_empty());

    let count = g.node_count().await.unwrap();
    assert_eq!(count, 2);

    let synth = g
        .list_edges_by_provenance(loopal_memory::Provenance::Synthesized)
        .await
        .unwrap();
    assert!(
        !synth.is_empty(),
        "expected synthesized edges from slug + tfidf"
    );
}

#[tokio::test]
async fn missing_memory_dir_returns_zero_stats_without_panic() {
    let temp = tempfile::tempdir().unwrap();
    let memory_dir = temp.path().join(".loopal/memory");

    let g = MemoryGraph::in_memory().unwrap();
    let stats = scan_directory(&g, &memory_dir).await.unwrap();
    assert_eq!(stats.files_scanned, 0);
}

#[tokio::test]
async fn reindex_is_idempotent_under_repeated_scans() {
    let temp = tempfile::tempdir().unwrap();
    let memory_dir = temp.path().join(".loopal/memory");
    std::fs::create_dir_all(&memory_dir).unwrap();
    std::fs::write(
        memory_dir.join("a.md"),
        "---\nname: a\ntype: project\n---\nbody\n",
    )
    .unwrap();

    let g = MemoryGraph::in_memory().unwrap();
    scan_directory(&g, &memory_dir).await.unwrap();
    scan_directory(&g, &memory_dir).await.unwrap();
    scan_directory(&g, &memory_dir).await.unwrap();

    assert_eq!(g.node_count().await.unwrap(), 1);
}

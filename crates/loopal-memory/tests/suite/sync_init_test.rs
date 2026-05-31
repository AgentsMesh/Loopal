use loopal_memory::MemoryGraph;
use loopal_memory::sync::scan_directory;

#[tokio::test]
async fn scan_empty_directory_yields_zero_stats() {
    let g = MemoryGraph::in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let stats = scan_directory(&g, dir.path()).await.unwrap();
    assert_eq!(stats.files_scanned, 0);
    assert_eq!(stats.nodes_indexed, 0);
}

#[tokio::test]
async fn scan_missing_directory_succeeds_with_zero_files() {
    let g = MemoryGraph::in_memory().unwrap();
    let dir = std::path::PathBuf::from("/this/does/not/exist");
    let stats = scan_directory(&g, &dir).await.unwrap();
    assert_eq!(stats.files_scanned, 0);
}

#[tokio::test]
async fn scan_indexes_single_markdown_file() {
    let g = MemoryGraph::in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let file_path = dir.path().join("twitter-auto.md");
    std::fs::write(
        &file_path,
        "---\nname: Twitter Auto\ntype: project\n---\n\nbody content here\n",
    )
    .unwrap();

    let stats = scan_directory(&g, dir.path()).await.unwrap();
    assert_eq!(stats.files_scanned, 1);
    assert_eq!(stats.nodes_indexed, 1);

    let node = g.get_node("twitter-auto").await.unwrap().unwrap();
    assert_eq!(node.name, "Twitter Auto");
}

#[tokio::test]
async fn scan_indexes_multiple_files_with_edges() {
    let g = MemoryGraph::in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("a.md"),
        "---\nname: a\ntype: project\nrelated:\n  - b\n---\nbody\n",
    )
    .unwrap();
    std::fs::write(
        dir.path().join("b.md"),
        "---\nname: b\ntype: project\n---\nrefers to [[a]] inline\n",
    )
    .unwrap();

    let stats = scan_directory(&g, dir.path()).await.unwrap();
    assert_eq!(stats.files_scanned, 2);
    assert_eq!(stats.nodes_indexed, 2);
    assert!(stats.edges_indexed >= 2);
}

#[tokio::test]
async fn scan_runs_synthesizers_to_populate_co_occurs() {
    let g = MemoryGraph::in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    for slug in ["twitter-a", "twitter-b", "twitter-c"] {
        std::fs::write(
            dir.path().join(format!("{}.md", slug)),
            format!("---\nname: {}\ntype: project\n---\nbody\n", slug),
        )
        .unwrap();
    }

    let stats = scan_directory(&g, dir.path()).await.unwrap();
    assert_eq!(stats.files_scanned, 3);

    let synth = g
        .list_edges_by_provenance(loopal_memory::Provenance::Synthesized)
        .await
        .unwrap();
    assert_eq!(synth.len(), 3);
}

#[tokio::test]
async fn scan_skips_non_markdown_files() {
    let g = MemoryGraph::in_memory().unwrap();
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("real.md"), "---\nname: real\n---\nbody").unwrap();
    std::fs::write(dir.path().join("ignore.txt"), "not markdown").unwrap();
    std::fs::write(dir.path().join("ignore.json"), "{}").unwrap();

    let stats = scan_directory(&g, dir.path()).await.unwrap();
    assert_eq!(stats.files_scanned, 1);
}

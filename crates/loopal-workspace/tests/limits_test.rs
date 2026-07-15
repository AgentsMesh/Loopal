use loopal_workspace::WorkspaceService;
use loopal_workspace::types::{SearchParams, WorkspacePathParams, WriteFileParams};

#[tokio::test]
async fn oversized_file_is_rejected_and_preserved() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("large.txt");
    std::fs::write(&path, "guard").unwrap();
    std::fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_len(10_000_001)
        .unwrap();
    let service = WorkspaceService::new(dir.path()).unwrap();

    let read_error = service
        .read_file(path_params("large.txt"))
        .await
        .unwrap_err();
    assert_eq!(read_error.code, "file_too_large");
    let write_error = service
        .write_file(WriteFileParams {
            workspace_id: "local-workspace".into(),
            path: "large.txt".into(),
            content: "replacement".into(),
            expected_version: None,
        })
        .await
        .unwrap_err();
    assert_eq!(write_error.code, "file_too_large");
    assert_eq!(&std::fs::read(&path).unwrap()[..5], b"guard");
}

#[tokio::test]
async fn search_preview_is_bounded_and_reports_truncation() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("long.txt"),
        format!("{}needle\n", "界".repeat(2_000)),
    )
    .unwrap();
    let service = WorkspaceService::new(dir.path()).unwrap();
    let result = service
        .search(SearchParams {
            workspace_id: "local-workspace".into(),
            query: "needle".into(),
            glob: None,
            max_results: 10,
        })
        .await
        .unwrap();
    assert!(result.truncated);
    assert!(result.matches[0].preview.len() <= 4_000);
}

fn path_params(path: &str) -> WorkspacePathParams {
    WorkspacePathParams {
        workspace_id: "local-workspace".into(),
        path: path.into(),
    }
}

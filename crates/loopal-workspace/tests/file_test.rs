use loopal_workspace::WorkspaceService;
use loopal_workspace::types::{SearchParams, WorkspacePathParams, WriteFileParams};

#[tokio::test]
async fn cas_read_list_and_search_are_root_scoped() {
    let dir = tempfile::tempdir().unwrap();
    let service = WorkspaceService::new(dir.path()).unwrap();
    let created = service
        .write_file(WriteFileParams {
            workspace_id: "local-workspace".into(),
            path: "src/main.rs".into(),
            content: "fn main() { println!(\"needle\"); }\n".into(),
            expected_version: None,
        })
        .await
        .unwrap();
    assert_eq!(created.language_id, "rust");
    let document = service
        .read_file(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "src/main.rs".into(),
        })
        .await
        .unwrap();
    assert_eq!(document.version, created.version);
    let listing = service
        .list_directory(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "src".into(),
        })
        .await
        .unwrap();
    assert_eq!(listing.entries[0].path, "src/main.rs");
    let result = service
        .search(SearchParams {
            workspace_id: "local-workspace".into(),
            query: "needle".into(),
            glob: Some("**/*.rs".into()),
            max_results: 20,
        })
        .await
        .unwrap();
    assert_eq!(result.matches[0].line, 1);
    assert_eq!(result.matches[0].column, 23);
    let conflict = service
        .write_file(WriteFileParams {
            workspace_id: "local-workspace".into(),
            path: "src/main.rs".into(),
            content: "changed".into(),
            expected_version: Some("stale".into()),
        })
        .await
        .unwrap_err();
    assert_eq!(conflict.code, "version_conflict");
    let escaped = service
        .read_file(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "../secret".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(escaped.code, "path_outside_root");
}

#[cfg(unix)]
#[tokio::test]
async fn symlink_escape_is_denied() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("secret"), "nope").unwrap();
    symlink(outside.path(), root.path().join("link")).unwrap();
    let service = WorkspaceService::new(root.path()).unwrap();
    let error = service
        .read_file(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "link/secret".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "path_outside_root");
}

#[tokio::test]
async fn concurrent_cas_allows_exactly_one_writer() {
    let dir = tempfile::tempdir().unwrap();
    let service = WorkspaceService::new(dir.path()).unwrap();
    let created = service
        .write_file(WriteFileParams {
            workspace_id: "local-workspace".into(),
            path: "shared.txt".into(),
            content: "zero".into(),
            expected_version: None,
        })
        .await
        .unwrap();
    let left = service.clone();
    let right = service.clone();
    let expected_left = created.version.clone();
    let expected_right = created.version;
    let (left, right) = tokio::join!(
        left.write_file(WriteFileParams {
            workspace_id: "local-workspace".into(),
            path: "shared.txt".into(),
            content: "left".into(),
            expected_version: Some(expected_left),
        }),
        right.write_file(WriteFileParams {
            workspace_id: "local-workspace".into(),
            path: "shared.txt".into(),
            content: "right".into(),
            expected_version: Some(expected_right),
        }),
    );
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let error = left.err().or_else(|| right.err()).unwrap();
    assert_eq!(error.code, "version_conflict");
}

#[tokio::test]
async fn unknown_workspace_is_rejected_before_io() {
    let dir = tempfile::tempdir().unwrap();
    let service = WorkspaceService::new(dir.path()).unwrap();
    let error = service
        .read_file(WorkspacePathParams {
            workspace_id: "remote-workspace".into(),
            path: "missing".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "workspace_not_found");
}

#[tokio::test]
async fn readonly_file_cannot_be_replaced_by_atomic_write() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("readonly.txt");
    std::fs::write(&path, "locked").unwrap();
    let original_permissions = std::fs::metadata(&path).unwrap().permissions();
    let mut permissions = original_permissions.clone();
    permissions.set_readonly(true);
    std::fs::set_permissions(&path, permissions).unwrap();
    let service = WorkspaceService::new(dir.path()).unwrap();
    let document = service
        .read_file(WorkspacePathParams {
            workspace_id: "local-workspace".into(),
            path: "readonly.txt".into(),
        })
        .await
        .unwrap();
    let error = service
        .write_file(WriteFileParams {
            workspace_id: "local-workspace".into(),
            path: "readonly.txt".into(),
            content: "replacement".into(),
            expected_version: Some(document.version),
        })
        .await
        .unwrap_err();
    assert_eq!(error.code, "readonly_file");
    std::fs::set_permissions(&path, original_permissions).unwrap();
}

use loopal_error::StorageError;
use loopal_storage::{FileResourceStore, ResourceStore};
use tempfile::tempdir;

fn resource_path(base: &std::path::Path, session: &str, id: &str) -> std::path::PathBuf {
    base.join("sessions")
        .join(session)
        .join("resources")
        .join(id)
}

fn png_bytes(fill: u8, len: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.resize(len.max(bytes.len()), fill);
    bytes
}

#[tokio::test]
async fn resource_ids_cannot_escape_the_resource_directory() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let error = store
        .read_bounded("sess", "../../outside", 32)
        .await
        .unwrap_err();
    assert!(matches!(error, StorageError::InvalidResourceId));
}

#[tokio::test]
async fn resource_sessions_reject_path_components() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let read = store
        .read_bounded("../sess", "deadbeefdeadbeefdeadbeefdeadbeef", 32)
        .await
        .unwrap_err();
    assert!(matches!(read, StorageError::InvalidPathComponent { .. }));
    let write = store
        .write("../sess", "image/png", b"value")
        .await
        .unwrap_err();
    assert!(matches!(write, StorageError::InvalidPathComponent { .. }));
    let delete = store.delete_session("../sess").await.unwrap_err();
    assert!(matches!(delete, StorageError::InvalidPathComponent { .. }));
}

#[tokio::test]
async fn bounded_read_rejects_oversized_resource_before_returning_bytes() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let bytes = png_bytes(7, 64);
    let id = store.write("sess", "image/png", &bytes).await.unwrap();
    let error = store.read_bounded("sess", &id, 63).await.unwrap_err();
    assert!(matches!(
        error,
        StorageError::ResourceByteLimitExceeded { max_bytes: 63 }
    ));
}

#[tokio::test]
async fn bounded_read_rejects_content_address_tampering() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let original = png_bytes(8, 32);
    let id = store.write("sess", "image/png", &original).await.unwrap();
    std::fs::write(resource_path(dir.path(), "sess", &id), b"tampered").unwrap();
    let error = store.read_bounded("sess", &id, 64).await.unwrap_err();
    assert!(matches!(error, StorageError::ResourceIntegrity));
}

#[tokio::test]
async fn valid_write_repairs_tampered_content_address() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let expected = png_bytes(9, 32);
    let id = store.write("sess", "image/png", &expected).await.unwrap();
    std::fs::write(resource_path(dir.path(), "sess", &id), b"tampered").unwrap();
    assert_eq!(
        store.write("sess", "image/png", &expected).await.unwrap(),
        id
    );
    assert_eq!(
        store
            .read_bounded("sess", &id, expected.len())
            .await
            .unwrap(),
        expected
    );
}

#[tokio::test]
async fn write_rejects_unknown_and_mismatched_image_payloads() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());

    for (mime, bytes) in [
        ("image/png", b"not-an-image".as_slice()),
        ("image/jpeg", png_bytes(11, 32).as_slice()),
    ] {
        let error = store.write("sess", mime, bytes).await.unwrap_err();
        assert!(matches!(error, StorageError::ResourceIntegrity));
    }
    assert!(!dir.path().join("sessions").exists());
}

#[cfg(unix)]
#[tokio::test]
async fn write_creates_and_repairs_private_resource_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let expected = png_bytes(12, 32);
    let id = store.write("sess", "image/png", &expected).await.unwrap();
    let path = resource_path(dir.path(), "sess", &id);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );

    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(
        store.write("sess", "image/png", &expected).await.unwrap(),
        id
    );
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[cfg(unix)]
#[tokio::test]
async fn resource_symlink_is_rejected_without_touching_target() {
    use std::os::unix::fs::symlink;

    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let expected = png_bytes(10, 32);
    let id = store.write("sess", "image/png", &expected).await.unwrap();
    let path = resource_path(dir.path(), "sess", &id);
    std::fs::remove_file(&path).unwrap();
    let target = dir.path().join("outside");
    std::fs::write(&target, b"outside").unwrap();
    symlink(&target, &path).unwrap();

    let read = store.read_bounded("sess", &id, 64).await.unwrap_err();
    assert!(matches!(read, StorageError::ResourceIntegrity));
    let write = store
        .write("sess", "image/png", &expected)
        .await
        .unwrap_err();
    assert!(matches!(write, StorageError::ResourceIntegrity));
    assert_eq!(std::fs::read(target).unwrap(), b"outside");
}

use loopal_storage::{FileResourceStore, ResourceStore};
use tempfile::tempdir;

fn png_bytes(fill: u8, len: usize) -> Vec<u8> {
    let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
    bytes.resize(len.max(bytes.len()), fill);
    bytes
}

#[test]
fn constructs_from_global_config_directory() {
    assert!(FileResourceStore::new().is_ok());
}

#[tokio::test]
async fn write_then_read_round_trip() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let bytes = png_bytes(1, 32);
    let id = store.write("sess-a", "image/png", &bytes).await.unwrap();
    let read = store
        .read_bounded("sess-a", &id, bytes.len())
        .await
        .unwrap();
    assert_eq!(read, bytes);
}

#[tokio::test]
async fn write_same_content_returns_same_id() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let bytes = png_bytes(0, 4096);
    let id1 = store.write("sess-a", "image/png", &bytes).await.unwrap();
    let id2 = store.write("sess-a", "image/png", &bytes).await.unwrap();
    assert_eq!(id1, id2);
}

#[tokio::test]
async fn write_different_content_returns_different_ids() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let id1 = store
        .write("sess-a", "image/png", &png_bytes(1, 16))
        .await
        .unwrap();
    let id2 = store
        .write("sess-a", "image/png", &png_bytes(2, 16))
        .await
        .unwrap();
    assert_ne!(id1, id2);
}

#[tokio::test]
async fn delete_session_removes_resources() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let id = store
        .write("sess-doomed", "image/png", &png_bytes(3, 16))
        .await
        .unwrap();
    store.delete_session("sess-doomed").await.unwrap();
    let err = store
        .read_bounded("sess-doomed", &id, usize::MAX)
        .await
        .unwrap_err();
    assert!(matches!(err, loopal_error::StorageError::Io(_)));
}

#[tokio::test]
async fn delete_session_when_no_resources_is_noop() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    store.delete_session("never-created").await.unwrap();
}

#[tokio::test]
async fn read_missing_returns_io_error() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let err = store
        .read_bounded("sess", "deadbeefdeadbeefdeadbeefdeadbeef", 1024)
        .await
        .unwrap_err();
    assert!(matches!(err, loopal_error::StorageError::Io(_)));
}

#[tokio::test]
async fn write_rejects_unsupported_media_type() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let err = store
        .write("sess", "application/octet-stream", b"x")
        .await
        .unwrap_err();
    assert!(matches!(err, loopal_error::StorageError::Serialization(_)));
}

#[tokio::test]
async fn write_returns_32_char_hex_id() {
    let dir = tempdir().unwrap();
    let store = FileResourceStore::with_base_dir(dir.path().to_path_buf());
    let id = store
        .write("sess", "image/png", &png_bytes(4, 16))
        .await
        .unwrap();
    assert_eq!(id.len(), 32);
    assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
}

#[tokio::test]
async fn concurrent_writes_of_same_content_yield_one_file() {
    let dir = tempdir().unwrap();
    let store = std::sync::Arc::new(FileResourceStore::with_base_dir(dir.path().to_path_buf()));
    let payload = png_bytes(42, 8192);
    let mut handles = Vec::new();
    for _ in 0..8 {
        let s = store.clone();
        let p = payload.clone();
        handles.push(tokio::spawn(async move {
            s.write("sess-c", "image/png", &p).await.unwrap()
        }));
    }
    let mut ids = Vec::new();
    for h in handles {
        ids.push(h.await.unwrap());
    }
    let first = ids[0].clone();
    for id in &ids {
        assert_eq!(id, &first);
    }
    let read = store
        .read_bounded("sess-c", &first, payload.len())
        .await
        .unwrap();
    assert_eq!(read, payload);
}

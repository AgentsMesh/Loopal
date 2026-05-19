use std::sync::Arc;

use loopal_backend::{LocalBackend, ResourceLimits};
use loopal_error::ToolIoError;
use loopal_tool_api::Backend;
use tempfile::tempdir;
use tokio::fs;

fn unique_session_id() -> String {
    format!("test-{}", uuid::Uuid::new_v4().simple())
}

fn make_backend(cwd: &std::path::Path) -> Arc<LocalBackend> {
    LocalBackend::new(
        cwd.to_path_buf(),
        None,
        ResourceLimits::default(),
        unique_session_id(),
    )
}

fn minimal_png(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"\x89PNG\r\n\x1a\n");
    v.extend_from_slice(&[0, 0, 0, 13]);
    v.extend_from_slice(b"IHDR");
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&[8, 6, 0, 0, 0]);
    v.extend_from_slice(&[0, 0, 0, 0]);
    v
}

fn minimal_jpeg(w: u16, h: u16) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(&[0xFF, 0xD8, 0xFF]);
    v.extend_from_slice(&[0xC0, 0, 17, 8]);
    v.extend_from_slice(&h.to_be_bytes());
    v.extend_from_slice(&w.to_be_bytes());
    v.extend_from_slice(&[3, 1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
    v.extend_from_slice(&[0xFF, 0xD9]);
    v
}

fn minimal_gif(w: u16, h: u16) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"GIF89a");
    v.extend_from_slice(&w.to_le_bytes());
    v.extend_from_slice(&h.to_le_bytes());
    v.extend_from_slice(&[0, 0, 0]);
    v
}

fn minimal_webp_vp8l(w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::new();
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&[20, 0, 0, 0]);
    v.extend_from_slice(b"WEBP");
    v.extend_from_slice(b"VP8L");
    v.extend_from_slice(&[8, 0, 0, 0]);
    v.push(0x2F);
    let wm = w - 1;
    let hm = h - 1;
    let b0 = (wm & 0xff) as u8;
    let b1 = (((wm >> 8) & 0x3f) | ((hm & 0x03) << 6)) as u8;
    let b2 = ((hm >> 2) & 0xff) as u8;
    let b3 = ((hm >> 10) & 0x0f) as u8;
    v.extend_from_slice(&[b0, b1, b2, b3]);
    v
}

#[tokio::test]
async fn read_image_png_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a.png");
    fs::write(&path, minimal_png(64, 48)).await.unwrap();
    let backend = make_backend(dir.path());
    let img = backend
        .read_image(path.to_str().unwrap())
        .await
        .expect("must read png");
    assert_eq!(img.media_type, "image/png");
    assert_eq!(img.dimensions, (64, 48));
    assert!(!img.data.is_empty());
    assert!(img.byte_size > 0);
}

#[tokio::test]
async fn read_image_jpeg_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a.jpg");
    fs::write(&path, minimal_jpeg(100, 200)).await.unwrap();
    let backend = make_backend(dir.path());
    let img = backend.read_image(path.to_str().unwrap()).await.unwrap();
    assert_eq!(img.media_type, "image/jpeg");
    assert_eq!(img.dimensions, (100, 200));
}

#[tokio::test]
async fn read_image_gif_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a.gif");
    fs::write(&path, minimal_gif(16, 32)).await.unwrap();
    let backend = make_backend(dir.path());
    let img = backend.read_image(path.to_str().unwrap()).await.unwrap();
    assert_eq!(img.media_type, "image/gif");
    assert_eq!(img.dimensions, (16, 32));
}

#[tokio::test]
async fn read_image_webp_vp8l_round_trip() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("a.webp");
    fs::write(&path, minimal_webp_vp8l(5, 10)).await.unwrap();
    let backend = make_backend(dir.path());
    let img = backend.read_image(path.to_str().unwrap()).await.unwrap();
    assert_eq!(img.media_type, "image/webp");
    assert_eq!(img.dimensions, (5, 10));
}

#[tokio::test]
async fn read_image_rejects_oversized_file() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("big.png");
    let mut payload = minimal_png(1, 1);
    payload.resize(11 * 1024 * 1024, 0);
    fs::write(&path, payload).await.unwrap();
    let backend = make_backend(dir.path());
    let err = backend
        .read_image(path.to_str().unwrap())
        .await
        .unwrap_err();
    assert!(matches!(err, ToolIoError::TooLarge { .. }), "got {err:?}");
}

#[tokio::test]
async fn read_image_rejects_oversized_pixels() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("huge.png");
    fs::write(&path, minimal_png(8193, 8193)).await.unwrap();
    let backend = make_backend(dir.path());
    let err = backend
        .read_image(path.to_str().unwrap())
        .await
        .unwrap_err();
    match err {
        ToolIoError::Other(msg) => assert!(msg.contains("pixels")),
        other => panic!("expected Other(pixels), got {other:?}"),
    }
}

#[tokio::test]
async fn read_image_sniffs_by_magic_not_extension() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("fake.png");
    fs::write(&path, minimal_jpeg(7, 9)).await.unwrap();
    let backend = make_backend(dir.path());
    let img = backend.read_image(path.to_str().unwrap()).await.unwrap();
    assert_eq!(img.media_type, "image/jpeg");
    assert_eq!(img.dimensions, (7, 9));
}

#[tokio::test]
async fn read_image_rejects_unknown_format() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("not_image.txt");
    fs::write(&path, b"plain text content not an image")
        .await
        .unwrap();
    let backend = make_backend(dir.path());
    let err = backend
        .read_image(path.to_str().unwrap())
        .await
        .unwrap_err();
    match err {
        ToolIoError::Other(msg) => assert!(msg.contains("unsupported")),
        other => panic!("expected Other(unsupported), got {other:?}"),
    }
}

#[tokio::test]
async fn read_image_rejects_truncated_data() {
    let dir = tempdir().unwrap();
    let path = dir.path().join("trunc.png");
    fs::write(&path, b"\x89PNG\r\n\x1a\n").await.unwrap();
    let backend = make_backend(dir.path());
    let err = backend
        .read_image(path.to_str().unwrap())
        .await
        .unwrap_err();
    match err {
        ToolIoError::Other(msg) => assert!(msg.contains("dimensions") || msg.contains("malformed")),
        other => panic!("expected Other(dimensions), got {other:?}"),
    }
}

#[tokio::test]
async fn read_image_rejects_missing_file() {
    let dir = tempdir().unwrap();
    let backend = make_backend(dir.path());
    let err = backend
        .read_image(dir.path().join("nope.png").to_str().unwrap())
        .await
        .unwrap_err();
    assert!(
        matches!(
            err,
            ToolIoError::Io(_) | ToolIoError::NotFound(_) | ToolIoError::PathDenied(_)
        ),
        "got {err:?}"
    );
}

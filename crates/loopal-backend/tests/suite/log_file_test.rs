use std::sync::Arc;

use loopal_backend::{LineSink, create_log_file, flush_writer, read_lines_into_sink};
use loopal_tool_api::{HeadTail, OutputTail, StderrCappedBuffer};
use parking_lot::Mutex as PlMutex;

use super::log_file_test_support::unique_session_id;

#[tokio::test]
async fn create_log_file_path_is_in_tmp_and_unique() {
    let sid = unique_session_id();
    let (p1, _w1) = create_log_file(&sid).await.unwrap();
    let (p2, _w2) = create_log_file(&sid).await.unwrap();
    assert!(p1.starts_with(std::env::temp_dir().join("loopal").join(&sid)));
    assert!(p1.extension().unwrap() == "log");
    assert_ne!(p1, p2, "uuid must produce unique paths");
    assert!(
        tokio::fs::metadata(&p1).await.is_ok(),
        "file must exist after create"
    );
}

#[tokio::test]
async fn read_lines_into_sink_stdout_writes_unprefixed_and_pushes_head_tail() {
    let (path, writer) = create_log_file(&unique_session_id()).await.unwrap();
    let writer = Arc::new(writer);
    let head_tail = Arc::new(HeadTail::new(10, 10));

    let (rx, tx) = tokio::io::duplex(1024);
    use tokio::io::AsyncWriteExt;
    let mut tx = tx;
    tokio::spawn(async move {
        tx.write_all(b"line1\nline2\n").await.unwrap();
    });

    read_lines_into_sink(
        rx,
        writer.clone(),
        LineSink::Stdout(head_tail.clone()),
        None,
    )
    .await;
    flush_writer(&writer).await;

    let on_disk = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(on_disk, "line1\nline2\n");
    let preview = head_tail.render_preview();
    assert_eq!(preview, "line1\nline2");
}

#[tokio::test]
async fn read_lines_into_sink_stderr_writes_err_prefix_to_file() {
    let (path, writer) = create_log_file(&unique_session_id()).await.unwrap();
    let writer = Arc::new(writer);
    let stderr_buf = Arc::new(PlMutex::new(StderrCappedBuffer::new()));

    let (rx, tx) = tokio::io::duplex(1024);
    use tokio::io::AsyncWriteExt;
    let mut tx = tx;
    tokio::spawn(async move {
        tx.write_all(b"warning\nerror\n").await.unwrap();
    });

    read_lines_into_sink(
        rx,
        writer.clone(),
        LineSink::Stderr(stderr_buf.clone()),
        None,
    )
    .await;
    flush_writer(&writer).await;

    let on_disk = tokio::fs::read_to_string(&path).await.unwrap();
    assert_eq!(on_disk, "[err] warning\n[err] error\n");
    let captured = stderr_buf.lock().snapshot();
    assert_eq!(captured, "warning\nerror\n");
}

#[tokio::test]
async fn read_lines_into_sink_pushes_progress_tail_when_present() {
    let (_path, writer) = create_log_file(&unique_session_id()).await.unwrap();
    let writer = Arc::new(writer);
    let head_tail = Arc::new(HeadTail::new(10, 10));
    let progress = Arc::new(OutputTail::new(10));

    let (rx, tx) = tokio::io::duplex(1024);
    use tokio::io::AsyncWriteExt;
    let mut tx = tx;
    tokio::spawn(async move {
        tx.write_all(b"alpha\nbeta\n").await.unwrap();
    });

    read_lines_into_sink(
        rx,
        writer.clone(),
        LineSink::Stdout(head_tail.clone()),
        Some(progress.clone()),
    )
    .await;
    flush_writer(&writer).await;
    let snap = progress.snapshot();
    assert!(snap.contains("alpha"));
    assert!(snap.contains("beta"));
}

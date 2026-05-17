use std::sync::Arc;
use std::time::Duration;

use loopal_backend::shell::{SpawnedBackgroundData, exec_background, exec_command};
use loopal_backend::shell_stream::{TimedOutProcessData, exec_command_streaming};
use loopal_backend::{LineSink, create_log_file, flush_writer, read_lines_into_sink};
use loopal_tool_api::ExecOutcome;
use loopal_tool_api::backend_types::EnvOverride;
use loopal_tool_api::{HeadTail, OutputTail, StderrCappedBuffer};
use parking_lot::Mutex as PlMutex;

#[tokio::test]
async fn create_log_file_path_is_in_tmp_and_unique() {
    let (p1, _w1) = create_log_file("test-session").await.unwrap();
    let (p2, _w2) = create_log_file("test-session").await.unwrap();
    assert!(p1.starts_with(std::env::temp_dir().join("loopal").join("test-session")));
    assert!(p1.extension().unwrap() == "log");
    assert_ne!(p1, p2, "uuid must produce unique paths");
    assert!(
        tokio::fs::metadata(&p1).await.is_ok(),
        "file must exist after create"
    );
}

#[tokio::test]
async fn read_lines_into_sink_stdout_writes_unprefixed_and_pushes_head_tail() {
    let (path, writer) = create_log_file("test-session").await.unwrap();
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
    let (path, writer) = create_log_file("test-session").await.unwrap();
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
    let (_path, writer) = create_log_file("test-session").await.unwrap();
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

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_writes_log_file_and_returns_path() {
    let cwd = std::env::temp_dir();

    let result = exec_command(
        &cwd,
        None,
        "echo SHORT_OUT",
        &EnvOverride::default(),
        Duration::from_secs(5),
        "test-session",
    )
    .await
    .unwrap();

    assert_eq!(result.exit_code, 0);
    assert!(result.stdout.contains("SHORT_OUT"));
    assert_eq!(result.stderr, "");
    assert!(!result.stdout_truncated);
    assert!(!result.stderr_truncated);
    assert!(result.log_path.starts_with(std::env::temp_dir()));
    let on_disk = tokio::fs::read_to_string(&result.log_path).await.unwrap();
    assert!(on_disk.contains("SHORT_OUT"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_interleaves_stderr_with_err_prefix_in_file() {
    let cwd = std::env::temp_dir();

    let result = exec_command(
        &cwd,
        None,
        "echo OUT_LINE; echo ERR_LINE >&2",
        &EnvOverride::default(),
        Duration::from_secs(5),
        "test-session",
    )
    .await
    .unwrap();
    assert!(result.stdout.contains("OUT_LINE"));
    assert!(result.stderr.contains("ERR_LINE"));
    let on_disk = tokio::fs::read_to_string(&result.log_path).await.unwrap();
    assert!(on_disk.contains("OUT_LINE"));
    assert!(on_disk.contains("[err] ERR_LINE"));
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_truncated_flags_for_long_stdout() {
    let cwd = std::env::temp_dir();

    let cmd = r#"for i in $(seq 1 200); do echo "L$i"; done"#;
    let result = exec_command(
        &cwd,
        None,
        cmd,
        &EnvOverride::default(),
        Duration::from_secs(10),
        "test-session",
    )
    .await
    .unwrap();
    assert!(result.stdout_truncated);
    assert!(result.stdout.contains("L1"));
    assert!(result.stdout.contains("L200"));
    assert!(result.stdout.contains("lines elided"));
    let on_disk = tokio::fs::read_to_string(&result.log_path).await.unwrap();
    assert!(on_disk.lines().count() >= 200);
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_background_creates_log_path_in_spawned_data() {
    let cwd = std::env::temp_dir();
    let data = exec_background(
        &cwd,
        None,
        "sleep 0.5",
        &EnvOverride::default(),
        "test-session",
    )
    .await
    .unwrap();
    let SpawnedBackgroundData {
        spawned,
        log_path,
        head_tail: _,
        stderr_buf: _,
        drainers: _,
    } = data;
    assert!(log_path.starts_with(std::env::temp_dir()));
    assert!(tokio::fs::metadata(&log_path).await.is_ok());
    let mut child = spawned.child;
    let _ = child.start_kill();
    let _ = child.wait().await;
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_streaming_completed_path_returns_log_path() {
    let cwd = std::env::temp_dir();

    let tail = Arc::new(OutputTail::new(20));
    let outcome = exec_command_streaming(
        &cwd,
        None,
        "echo STREAM_DONE",
        &EnvOverride::default(),
        Duration::from_secs(5),
        tail,
        "test-session",
    )
    .await
    .unwrap();
    match outcome {
        ExecOutcome::Completed(result) => {
            assert!(result.stdout.contains("STREAM_DONE"));
            assert!(result.log_path.starts_with(std::env::temp_dir()));
            let on_disk = tokio::fs::read_to_string(&result.log_path).await.unwrap();
            assert!(on_disk.contains("STREAM_DONE"));
        }
        ExecOutcome::TimedOut { .. } => panic!("expected completed, got timeout"),
    }
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_streaming_timeout_handle_carries_log_path() {
    let cwd = std::env::temp_dir();

    let tail = Arc::new(OutputTail::new(20));
    let outcome = exec_command_streaming(
        &cwd,
        None,
        "echo PARTIAL; sleep 5",
        &EnvOverride::default(),
        Duration::from_millis(500),
        tail,
        "test-session",
    )
    .await
    .unwrap();
    match outcome {
        ExecOutcome::TimedOut { handle, .. } => {
            let data = handle.0.downcast::<TimedOutProcessData>().ok().unwrap();
            assert!(data.log_path.starts_with(std::env::temp_dir()));
            tokio::time::sleep(Duration::from_millis(2000)).await;
            let on_disk = match tokio::fs::read_to_string(&data.log_path).await {
                Ok(s) => s,
                Err(e) => panic!("log file {} not readable: {e}", data.log_path.display()),
            };
            assert!(on_disk.contains("PARTIAL"));
            let mut child = data.spawned.child;
            let _ = child.start_kill();
            let _ = child.wait().await;
        }
        ExecOutcome::Completed(_) => panic!("expected timeout, got completed"),
    }
}

#[tokio::test]
#[cfg(not(windows))]
async fn exec_command_timeout_kills_process_group() {
    let cwd = std::env::temp_dir();

    let probe = uuid_lite();
    let pid_file = std::env::temp_dir().join(format!("loopal_pgid_{probe}"));
    let cmd = format!(
        "echo $$ > {} ; sleep 30 & sleep 30 & wait",
        pid_file.display()
    );
    let result = exec_command(
        &cwd,
        None,
        &cmd,
        &EnvOverride::default(),
        Duration::from_millis(400),
        "test-session",
    )
    .await;
    assert!(matches!(result, Err(loopal_error::ToolIoError::Timeout(_))));

    let pgid: i32 = match tokio::fs::read_to_string(&pid_file).await {
        Ok(s) => s.trim().parse().unwrap_or(0),
        Err(_) => return,
    };
    if pgid == 0 {
        return;
    }

    tokio::time::sleep(Duration::from_millis(700)).await;
    let count = count_pids_in_pgroup(pgid);
    assert_eq!(
        count, 0,
        "expected no process in pgid {pgid} after timeout, got {count}"
    );
    let _ = tokio::fs::remove_file(&pid_file).await;
}

fn uuid_lite() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

#[cfg(not(windows))]
fn count_pids_in_pgroup(pgid: i32) -> usize {
    let out = match std::process::Command::new("ps")
        .args(["-o", "pgid="])
        .arg("-g")
        .arg(pgid.to_string())
        .output()
    {
        Ok(o) => o,
        Err(_) => return 0,
    };
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
}

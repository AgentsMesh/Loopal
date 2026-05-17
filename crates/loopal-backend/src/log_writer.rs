use std::path::PathBuf;
use std::sync::Arc;

use loopal_error::ToolIoError;
use loopal_tool_api::output_tail::OutputTail;
use loopal_tool_api::{HeadTail, StderrCappedBuffer};
use parking_lot::Mutex as PlMutex;
use tokio::fs::File;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

use crate::tmp_cleanup::{is_valid_session_id, session_tmp_root};

// reason: unbuffered File so each line is visible to file tailers immediately.
pub type LogWriter = TokioMutex<File>;

pub fn session_bash_dir(session_id: &str) -> PathBuf {
    session_tmp_root(session_id).join("bash")
}

pub async fn create_log_file(session_id: &str) -> Result<(PathBuf, LogWriter), ToolIoError> {
    // reason: reject path-traversal session ids before they create files
    // outside `$TMPDIR/loopal/{id}/bash/`. Mirrors cleanup_session_tmp's
    // guard so both ends share the same trust boundary.
    if !is_valid_session_id(session_id) {
        return Err(ToolIoError::ExecFailed(format!(
            "invalid session id for log file: {session_id:?}"
        )));
    }
    let dir = session_bash_dir(session_id);
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| ToolIoError::ExecFailed(format!("create log dir {}: {e}", dir.display())))?;
    let id = Uuid::new_v4().simple().to_string();
    let path = dir.join(format!("{id}.log"));
    let file = File::create(&path)
        .await
        .map_err(|e| ToolIoError::ExecFailed(format!("create log file {}: {e}", path.display())))?;
    Ok((path, TokioMutex::new(file)))
}

#[derive(Clone)]
pub enum LineSink {
    Stdout(Arc<HeadTail>),
    Stderr(Arc<PlMutex<StderrCappedBuffer>>),
}

// reason: file write_all MUST precede in-memory push so the file is the
// strict ground truth — readers that see a line in HeadTail/StderrBuf are
// guaranteed to find it in the log file too.
pub async fn read_lines_into_sink<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    writer: Arc<LogWriter>,
    sink: LineSink,
    progress_tail: Option<Arc<OutputTail>>,
) {
    let mut br = BufReader::new(reader);
    let mut line = String::new();
    loop {
        line.clear();
        match br.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => {
                let trimmed = line.trim_end_matches('\n').to_string();
                let to_file = match &sink {
                    LineSink::Stdout(_) => line.clone(),
                    LineSink::Stderr(_) => format!("[err] {line}"),
                };
                {
                    let mut w = writer.lock().await;
                    let _ = w.write_all(to_file.as_bytes()).await;
                }
                if let Some(t) = &progress_tail {
                    t.push_line(trimmed.clone());
                }
                match &sink {
                    LineSink::Stdout(ht) => ht.push_line(trimmed),
                    LineSink::Stderr(sb) => sb.lock().push_str(&line),
                }
            }
            Err(_) => break,
        }
    }
}

pub async fn flush_writer(writer: &LogWriter) {
    let mut w = writer.lock().await;
    let _ = w.flush().await;
}

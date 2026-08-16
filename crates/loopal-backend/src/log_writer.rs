use std::path::{Path, PathBuf};
use std::sync::Arc;

use loopal_error::ToolIoError;
use loopal_tool_api::{HeadTail, OutputTail, StderrCappedBuffer};
use parking_lot::Mutex as PlMutex;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex as TokioMutex;
use uuid::Uuid;

use crate::tmp_cleanup::{is_valid_session_id, loopal_tmp_root, session_tmp_root_in};

pub type LogWriter = TokioMutex<File>;

pub fn session_bash_dir(session_id: &str) -> PathBuf {
    session_bash_dir_in(&loopal_tmp_root(), session_id)
}

pub fn session_bash_dir_in(root: &Path, session_id: &str) -> PathBuf {
    session_tmp_root_in(root, session_id).join("bash")
}

pub async fn create_log_file(session_id: &str) -> Result<(PathBuf, LogWriter), ToolIoError> {
    create_log_file_in(&loopal_tmp_root(), session_id).await
}

pub async fn create_log_file_in(
    root: &Path,
    session_id: &str,
) -> Result<(PathBuf, LogWriter), ToolIoError> {
    if !is_valid_session_id(session_id) {
        return Err(ToolIoError::ExecFailed(format!(
            "invalid session id for log file: {session_id:?}"
        )));
    }
    let session_dir = session_tmp_root_in(root, session_id);
    let bash_dir = session_bash_dir_in(root, session_id);
    crate::private_log_file::prepare_directory(root).await?;
    crate::private_log_file::prepare_directory(&session_dir).await?;
    crate::private_log_file::prepare_directory(&bash_dir).await?;
    let id = Uuid::new_v4().simple().to_string();
    let path = bash_dir.join(format!("{id}.log"));
    let file = crate::private_log_file::create(&path).await?;
    Ok((path, TokioMutex::new(file)))
}

#[derive(Clone)]
pub enum LineSink {
    Stdout(Arc<HeadTail>),
    Stderr(Arc<PlMutex<StderrCappedBuffer>>),
}

pub async fn read_lines_into_sink<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    writer: Arc<LogWriter>,
    sink: LineSink,
    progress_tail: Option<Arc<OutputTail>>,
) {
    crate::log_capture::capture(reader, writer, sink, progress_tail).await;
}

pub async fn flush_writer(writer: &LogWriter) {
    let mut file = writer.lock().await;
    let _ = file.flush().await;
}

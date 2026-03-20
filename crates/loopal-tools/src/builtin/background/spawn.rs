use loopal_error::LoopalError;
use loopal_tool_api::{ToolContext, ToolResult};
use serde_json::Value;
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use tokio::io::AsyncReadExt;
use tokio::process::Command;

use super::{BackgroundTask, TaskStatus};

/// Spawn a command as a background task and return immediately with the task ID.
pub async fn spawn_background(
    command: &str,
    input: &Value,
    ctx: &ToolContext,
) -> Result<ToolResult, LoopalError> {
    let description = input["description"]
        .as_str()
        .unwrap_or(command)
        .to_string();

    let child = Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(&ctx.cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            LoopalError::Tool(loopal_error::ToolError::ExecutionFailed(e.to_string()))
        })?;

    let task_id = super::generate_task_id();
    let output_buf = Arc::new(Mutex::new(String::new()));
    let exit_code_buf = Arc::new(Mutex::new(None));
    let status_buf = Arc::new(Mutex::new(TaskStatus::Running));

    let task = BackgroundTask {
        output: Arc::clone(&output_buf),
        exit_code: Arc::clone(&exit_code_buf),
        status: Arc::clone(&status_buf),
        description,
        child: Arc::new(Mutex::new(Some(child))),
    };

    let child_handle = Arc::clone(&task.child);
    super::store()
        .lock()
        .unwrap()
        .insert(task_id.clone(), task);

    tokio::spawn(collect_output(
        child_handle,
        output_buf,
        exit_code_buf,
        status_buf,
    ));

    Ok(ToolResult::success(format!(
        "Background task started: {task_id}"
    )))
}

async fn collect_output(
    child_handle: Arc<Mutex<Option<tokio::process::Child>>>,
    output_buf: Arc<Mutex<String>>,
    exit_code_buf: Arc<Mutex<Option<i32>>>,
    status_buf: Arc<Mutex<TaskStatus>>,
) {
    let mut child = child_handle.lock().unwrap().take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();

    let (mut out_bytes, mut err_bytes) = (Vec::new(), Vec::new());
    let _ = tokio::join!(
        stdout.read_to_end(&mut out_bytes),
        stderr.read_to_end(&mut err_bytes),
    );

    let mut combined = String::from_utf8_lossy(&out_bytes).into_owned();
    if !err_bytes.is_empty() {
        if !combined.is_empty() {
            combined.push('\n');
        }
        combined.push_str(&String::from_utf8_lossy(&err_bytes));
    }

    *output_buf.lock().unwrap() = combined;

    let exit_status = child.wait().await.ok().and_then(|s| s.code());
    *exit_code_buf.lock().unwrap() = exit_status;

    let success = exit_status == Some(0);
    *status_buf.lock().unwrap() = if success {
        TaskStatus::Completed
    } else {
        TaskStatus::Failed
    };
}

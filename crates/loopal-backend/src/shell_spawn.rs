use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;

use loopal_config::ResolvedPolicy;
use loopal_error::ToolIoError;
use loopal_tool_api::backend_types::EnvOverride;
use loopal_tool_api::output_tail::OutputTail;
use loopal_tool_api::{HeadTail, StderrCappedBuffer};
use parking_lot::Mutex as PlMutex;
use tokio::process::{ChildStderr, ChildStdout, Command};
use tokio::task::JoinHandle;

use crate::log_writer::{LineSink, LogWriter, create_log_file, read_lines_into_sink};
use crate::process_group::{SpawnedChild, capture_pgid, configure_process_group};

pub(crate) const HEAD_LINES: usize = 25;
pub(crate) const TAIL_LINES: usize = 25;

pub struct SpawnedBackgroundData {
    pub spawned: SpawnedChild,
    pub log_path: std::path::PathBuf,
    pub head_tail: Arc<HeadTail>,
    pub stderr_buf: Arc<PlMutex<StderrCappedBuffer>>,
    /// Owns the reader tasks. The monitor awaits these on natural child
    /// exit so `head_tail`/`log_path` are fully populated before the task
    /// is marked terminal — fixes the race where `bg_output` could return
    /// a half-read preview.
    pub drainers: Vec<JoinHandle<()>>,
}

pub(crate) struct PreparedSpawn {
    pub spawned: SpawnedChild,
    pub stdout_pipe: Option<ChildStdout>,
    pub stderr_pipe: Option<ChildStderr>,
    pub log_path: std::path::PathBuf,
    pub log_writer: Arc<LogWriter>,
    pub head_tail: Arc<HeadTail>,
    pub stderr_buf: Arc<PlMutex<StderrCappedBuffer>>,
}

pub(crate) async fn prepare_spawn(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
    env_overrides: &EnvOverride,
    session_id: &str,
) -> Result<PreparedSpawn, ToolIoError> {
    let (program, args, env) = build_command(cwd, policy, command);
    let mut cmd = Command::new(&program);
    cmd.args(&args)
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    if let Some(env_map) = env {
        cmd.env_clear();
        for (k, v) in env_map {
            cmd.env(k, v);
        }
    }
    for (k, v) in &env_overrides.vars {
        cmd.env(k, v);
    }
    configure_process_group(&mut cmd);

    let mut child = cmd
        .spawn()
        .map_err(|e| ToolIoError::ExecFailed(format!("spawn failed: {e}")))?;
    let pgid = capture_pgid(&child);
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();

    let (log_path, log_writer) = create_log_file(session_id).await?;
    Ok(PreparedSpawn {
        spawned: SpawnedChild { child, pgid },
        stdout_pipe,
        stderr_pipe,
        log_path,
        log_writer: Arc::new(log_writer),
        head_tail: Arc::new(HeadTail::new(HEAD_LINES, TAIL_LINES)),
        stderr_buf: Arc::new(PlMutex::new(StderrCappedBuffer::new())),
    })
}

pub(crate) fn spawn_readers(
    stdout_pipe: Option<ChildStdout>,
    stderr_pipe: Option<ChildStderr>,
    log_writer: Arc<LogWriter>,
    head_tail: Arc<HeadTail>,
    stderr_buf: Arc<PlMutex<StderrCappedBuffer>>,
    progress_tail: Option<Arc<OutputTail>>,
) -> Vec<JoinHandle<()>> {
    let mut handles = Vec::new();
    if let Some(pipe) = stdout_pipe {
        let writer = log_writer.clone();
        let ht = head_tail;
        let tail = progress_tail.clone();
        handles.push(tokio::spawn(async move {
            read_lines_into_sink(pipe, writer, LineSink::Stdout(ht), tail).await
        }));
    }
    if let Some(pipe) = stderr_pipe {
        let writer = log_writer;
        let sb = stderr_buf;
        let tail = progress_tail;
        handles.push(tokio::spawn(async move {
            read_lines_into_sink(pipe, writer, LineSink::Stderr(sb), tail).await
        }));
    }
    handles
}

type EnvMap = std::collections::HashMap<String, String>;

pub(crate) fn build_command(
    cwd: &Path,
    policy: Option<&ResolvedPolicy>,
    command: &str,
) -> (String, Vec<String>, Option<EnvMap>) {
    if let Some(pol) = policy {
        let sc = loopal_sandbox::command_wrapper::wrap_command(pol, command, cwd);
        (sc.program, sc.args, Some(sc.env))
    } else if cfg!(windows) {
        let comspec = std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".into());
        (comspec, vec!["/C".into(), command.into()], None)
    } else {
        ("sh".into(), vec!["-c".into(), command.into()], None)
    }
}

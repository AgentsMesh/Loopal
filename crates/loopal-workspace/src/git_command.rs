use std::io::{self, Read};
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use loopal_protocol::META_HUB_TOKEN_ENV;

use crate::WorkspaceError;

pub(crate) const MAX_GIT_RESPONSE_BYTES: usize = 8 * 1024 * 1024;
const MAX_GIT_STDERR_BYTES: usize = 64 * 1024;

pub(crate) async fn run_git(cwd: PathBuf, args: Vec<String>) -> Result<Output, WorkspaceError> {
    execute(cwd, args, false).await
}

pub(crate) async fn read_git(cwd: PathBuf, args: Vec<String>) -> Result<Output, WorkspaceError> {
    execute(cwd, args, true).await
}

async fn execute(
    cwd: PathBuf,
    args: Vec<String>,
    readonly: bool,
) -> Result<Output, WorkspaceError> {
    let output = tokio::task::spawn_blocking(move || -> Result<Output, WorkspaceError> {
        let mut command = Command::new("git");
        command.args(args).current_dir(cwd);
        command.env_remove(META_HUB_TOKEN_ENV);
        if readonly {
            command.env("GIT_OPTIONAL_LOCKS", "0");
        }
        collect_output(command)
    })
    .await
    .map_err(WorkspaceError::io)??;
    if output.status.success() {
        Ok(output)
    } else {
        Err(WorkspaceError::new(
            "git_error",
            String::from_utf8_lossy(&output.stderr).trim(),
        ))
    }
}

fn collect_output(mut command: Command) -> Result<Output, WorkspaceError> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| WorkspaceError::io("git stdout unavailable"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| WorkspaceError::io("git stderr unavailable"))?;
    let stdout = std::thread::spawn(move || read_bounded(stdout, MAX_GIT_RESPONSE_BYTES));
    let stderr = std::thread::spawn(move || read_bounded(stderr, MAX_GIT_STDERR_BYTES));
    let status = child.wait()?;
    let stdout = join_reader(stdout)?;
    let stderr = join_reader(stderr)?;
    Ok(Output {
        status,
        stdout: require_bounded(stdout, MAX_GIT_RESPONSE_BYTES, "stdout")?,
        stderr: require_bounded(stderr, MAX_GIT_STDERR_BYTES, "stderr")?,
    })
}

fn read_bounded(reader: impl Read, limit: usize) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.take(limit as u64 + 1).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn join_reader(
    reader: std::thread::JoinHandle<io::Result<Vec<u8>>>,
) -> Result<Vec<u8>, WorkspaceError> {
    reader
        .join()
        .map_err(|_| WorkspaceError::io("git output reader panicked"))?
        .map_err(WorkspaceError::io)
}

fn require_bounded(bytes: Vec<u8>, limit: usize, stream: &str) -> Result<Vec<u8>, WorkspaceError> {
    if bytes.len() > limit {
        Err(WorkspaceError::new(
            "response_too_large",
            format!("git {stream} exceeded its response limit"),
        ))
    } else {
        Ok(bytes)
    }
}

use std::process::Stdio;
use std::time::Duration;

use anyhow::Context as _;
use tokio::io::{AsyncBufReadExt, AsyncReadExt as _, BufReader};
use tokio::process::{Child, Command};

use crate::cli::Cli;

const HANDSHAKE_PREFIX: &str = "LOOPAL_HUB ";
const HANDSHAKE_ERROR_PREFIX: &str = "LOOPAL_HUB_ERROR ";
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const HANDSHAKE_MAX_LINES: usize = 64;
// Cap on stdout bytes consumed before giving up. Real handshakes are
// ~150 bytes; runaway child output (broken shim, log spam) must not
// drag the parent into unbounded buffering.
const HANDSHAKE_MAX_BYTES: u64 = 64 * 1024;

pub struct HubHandshake {
    pub addr: String,
    pub token: String,
    pub root_session_id: String,
    /// Hub child process handle. `kill_on_drop(false)` is set, so dropping
    /// without `wait()` leaves the child running (the detach case). For
    /// `/exit` and `/kill-hub`, the caller awaits this handle to ensure the
    /// child fully exits before any worktree cleanup, avoiding the race
    /// where parent removes files while agent is still flushing them.
    pub child: Child,
}

pub async fn spawn_hub_subprocess(
    cli: &Cli,
    cwd: &std::path::Path,
    resume: Option<&str>,
) -> anyhow::Result<HubHandshake> {
    let exe = std::env::current_exe().context("locate current exe for hub spawn")?;

    let mut cmd = Command::new(&exe);
    cmd.arg("--hub-only");
    push_passthrough_args(&mut cmd, cli, resume);
    cmd.current_dir(cwd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::null());
    cmd.kill_on_drop(false);

    detach_from_tty(&mut cmd);

    let mut child = cmd.spawn().context("spawn hub-only subprocess")?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow::anyhow!("hub child stdout was not captured"))?;

    let reader = BufReader::new(stdout.take(HANDSHAKE_MAX_BYTES));
    let outcome = tokio::time::timeout(HANDSHAKE_TIMEOUT, read_handshake(reader))
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "hub child did not produce a handshake within {}s",
                HANDSHAKE_TIMEOUT.as_secs()
            )
        });
    let (addr, token, root_session_id) = match outcome.and_then(|res| res) {
        Ok(triple) => triple,
        Err(e) => {
            // Hub child is detached (setsid + kill_on_drop=false). On
            // handshake failure we MUST kill it explicitly or it lives
            // on as an unreachable orphan.
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(e);
        }
    };

    Ok(HubHandshake {
        addr,
        token,
        root_session_id,
        child,
    })
}

async fn read_handshake<R>(mut reader: BufReader<R>) -> anyhow::Result<(String, String, String)>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut skipped = Vec::new();
    for _ in 0..HANDSHAKE_MAX_LINES {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .await
            .context("read hub handshake line")?;
        if n == 0 {
            let context = format_skipped_lines(&skipped);
            anyhow::bail!("hub child closed stdout before sending handshake{context}");
        }
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if let Some(rest) = trimmed.strip_prefix(HANDSHAKE_ERROR_PREFIX) {
            anyhow::bail!("hub child failed to start: {rest}");
        }
        if let Some(rest) = trimmed.strip_prefix(HANDSHAKE_PREFIX) {
            return parse_handshake_fields(rest);
        }
        skipped.push(trimmed.to_string());
    }
    anyhow::bail!(
        "hub child wrote {HANDSHAKE_MAX_LINES} lines without recognised handshake prefix; \
         likely stdout pollution"
    );
}

fn parse_handshake_fields(rest: &str) -> anyhow::Result<(String, String, String)> {
    let mut parts = rest.splitn(3, ' ');
    let addr = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("malformed hub handshake (no addr): {rest:?}"))?;
    let token = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("malformed hub handshake (no token): {rest:?}"))?;
    let session_id = parts
        .next()
        .ok_or_else(|| anyhow::anyhow!("malformed hub handshake (no session_id): {rest:?}"))?;
    Ok((addr.to_string(), token.to_string(), session_id.to_string()))
}

fn format_skipped_lines(skipped: &[String]) -> String {
    if skipped.is_empty() {
        return String::new();
    }
    format!(
        " (saw {} non-handshake lines, last: {:?})",
        skipped.len(),
        skipped.last().unwrap()
    )
}

fn push_passthrough_args(cmd: &mut Command, cli: &Cli, resume: Option<&str>) {
    if let Some(model) = &cli.model {
        cmd.arg("--model").arg(model);
    }
    if let Some(perm) = &cli.permission {
        cmd.arg("--permission").arg(perm);
    }
    if cli.plan {
        cmd.arg("--plan");
    }
    if cli.no_sandbox {
        cmd.arg("--no-sandbox");
    }
    if cli.ephemeral {
        cmd.arg("--ephemeral");
    }
    if let Some(id) = resume {
        cmd.arg("--resume").arg(id);
    }
    for word in &cli.prompt {
        cmd.arg(word);
    }
}

#[cfg(unix)]
fn detach_from_tty(cmd: &mut Command) {
    unsafe extern "C" {
        fn setsid() -> i32;
    }
    unsafe {
        cmd.pre_exec(|| {
            if setsid() == -1 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

#[cfg(windows)]
fn detach_from_tty(cmd: &mut Command) {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    cmd.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

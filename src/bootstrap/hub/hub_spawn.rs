use std::process::Stdio;
use std::time::Duration;

use anyhow::Context as _;
use tokio::io::{AsyncBufReadExt, AsyncReadExt as _, BufReader};
use tokio::process::{Child, Command};

use loopal_ipc::HandshakeLine;

use crate::cli::Cli;

const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);
const HANDSHAKE_MAX_LINES: usize = 64;
// reason: real handshakes are ~150 bytes; runaway child output (broken shim,
// log spam) must not drag the parent into unbounded buffering.
const HANDSHAKE_MAX_BYTES: u64 = 64 * 1024;

pub struct HubHandshake {
    pub addr: String,
    pub token: String,
    pub root_session_id: String,
    /// The real TUI lease registered between ALIVE and READY.
    pub ui: super::attach_bridge::RegisteredUi,
    /// Hub child process handle. `kill_on_drop(false)` is set, so dropping
    /// without `wait()` leaves the child running (the detach case).
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
    cmd.args(build_hub_only_argv(cli, resume));
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
    let (addr, token, root_session_id, ui) = match outcome.and_then(|res| res) {
        Ok(handshake) => handshake,
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
        ui,
        child,
    })
}

async fn read_handshake<R>(
    mut reader: BufReader<R>,
) -> anyhow::Result<(String, String, String, super::attach_bridge::RegisteredUi)>
where
    R: tokio::io::AsyncRead + Unpin,
{
    let mut alive: Option<(String, String)> = None;
    let mut ready_session: Option<String> = None;
    let mut ui = None;
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
        match HandshakeLine::parse(&line) {
            Some(HandshakeLine::Error(rest)) => anyhow::bail!("hub child failed to start: {rest}"),
            Some(HandshakeLine::Alive { addr, token }) => {
                alive = Some((addr, token));
            }
            Some(HandshakeLine::Ready { session_id }) => {
                ready_session = Some(session_id);
            }
            Some(HandshakeLine::Legacy {
                addr,
                token,
                session_id,
            }) => {
                alive = Some((addr, token));
                ready_session = Some(session_id);
            }
            None => skipped.push(line.trim_end().to_string()),
        }
        if ui.is_none()
            && let Some((addr, token)) = &alive
        {
            ui = Some(super::attach_bridge::connect_and_register(addr, token).await?);
        }
        if let Some((addr, token)) = &alive
            && let Some(session_id) = &ready_session
            && let Some(ui) = ui.take()
        {
            return Ok((addr.clone(), token.clone(), session_id.clone(), ui));
        }
    }
    anyhow::bail!(
        "hub child wrote {HANDSHAKE_MAX_LINES} lines without recognised handshake prefix; \
         likely stdout pollution"
    );
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

fn build_hub_only_argv(cli: &Cli, resume: Option<&str>) -> Vec<std::ffi::OsString> {
    let mut argv = cli.child.to_args();
    argv.push("--require-ui-ready".into());
    if let Some(id) = resume {
        argv.push("--resume".into());
        argv.push(id.into());
    }
    for word in &cli.prompt {
        argv.push(word.into());
    }
    argv
}

#[cfg(test)]
#[path = "tests/hub_spawn_argv.rs"]
mod hub_spawn_argv_test;

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

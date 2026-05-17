use std::time::Duration;
use tokio::process::{Child, Command};

pub struct SpawnedChild {
    pub child: Child,
    pub pgid: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KillOutcome {
    Terminated,
    Killed,
    FallbackChild,
    KillFailed(String),
}

pub fn configure_process_group(cmd: &mut Command) {
    #[cfg(unix)]
    {
        unsafe {
            cmd.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(std::io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        cmd.creation_flags(CREATE_NEW_PROCESS_GROUP);
    }
}

pub fn capture_pgid(child: &Child) -> Option<i32> {
    child.id().map(|p| p as i32)
}

pub async fn kill_process_group(
    pgid: Option<i32>,
    child: &mut Child,
    sigterm_grace: Duration,
) -> KillOutcome {
    let Some(pgid) = pgid else {
        return fallback_kill(child).await;
    };
    kill_group_inner(pgid, child, sigterm_grace).await
}

#[cfg(unix)]
async fn kill_group_inner(pgid: i32, child: &mut Child, sigterm_grace: Duration) -> KillOutcome {
    // reason: SIGTERM grace lets well-behaved children (Node, Python, shell
    // scripts with EXIT traps) flush logs and clean up resources. Most common
    // runtimes respond well within ~500ms; anything that ignores SIGTERM gets
    // SIGKILL after grace. Configurable via BgTaskConfig::sigterm_grace_ms.
    unsafe {
        libc::killpg(pgid, libc::SIGTERM);
    }
    if tokio::time::timeout(sigterm_grace, child.wait())
        .await
        .is_ok()
    {
        return KillOutcome::Terminated;
    }
    let rc = unsafe { libc::killpg(pgid, libc::SIGKILL) };
    if rc != 0 {
        return KillOutcome::KillFailed(format!(
            "killpg(SIGKILL) failed: errno {}",
            std::io::Error::last_os_error()
        ));
    }
    KillOutcome::Killed
}

#[cfg(windows)]
async fn kill_group_inner(pgid: i32, child: &mut Child, sigterm_grace: Duration) -> KillOutcome {
    use windows_sys::Win32::System::Console::{CTRL_BREAK_EVENT, GenerateConsoleCtrlEvent};
    unsafe {
        GenerateConsoleCtrlEvent(CTRL_BREAK_EVENT, pgid as u32);
    }
    if tokio::time::timeout(sigterm_grace, child.wait())
        .await
        .is_ok()
    {
        return KillOutcome::Terminated;
    }
    match child.start_kill() {
        Err(e) => KillOutcome::KillFailed(e.to_string()),
        Ok(()) => KillOutcome::Killed,
    }
}

async fn fallback_kill(child: &mut Child) -> KillOutcome {
    if let Err(e) = child.start_kill() {
        return KillOutcome::KillFailed(e.to_string());
    }
    KillOutcome::FallbackChild
}

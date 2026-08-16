use std::io;
use std::process::ExitStatus;
use std::time::Duration;

use tokio::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

use super::unix_members::{has_live, wait_until_terminal};
use super::{KillOutcome, Termination};

#[cfg(test)]
#[path = "unix_tests.rs"]
mod tests;

const WAIT_POLL_INTERVAL: Duration = Duration::from_millis(10);

#[derive(Debug)]
pub(super) struct PlatformChild {
    child: Child,
    pgid: i32,
}

impl PlatformChild {
    pub(super) fn spawn(mut command: Command) -> io::Result<Self> {
        command.kill_on_drop(true);
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() < 0 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
        let child = command.spawn()?;
        let pgid = child
            .id()
            .and_then(|id| i32::try_from(id).ok())
            .filter(|id| *id > 1)
            .ok_or_else(|| io::Error::other("spawned process has no valid group id"))?;
        Ok(Self { child, pgid })
    }

    pub(super) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub(super) fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub(super) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    pub(super) async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.wait_for_root_exit().await?;
        if has_live(self.pgid).await? {
            signal_group(self.pgid, libc::SIGKILL)?;
            wait_until_terminal(self.pgid, None).await?;
        }
        self.child.wait().await
    }

    pub(super) async fn terminate(&mut self, grace: Duration) -> Termination {
        match has_live(self.pgid).await {
            Ok(false) => return self.reap(KillOutcome::Terminated, grace).await,
            Ok(true) => {}
            Err(error) => return failed("initial inspection", error),
        }
        if let Err(error) = signal_group(self.pgid, libc::SIGTERM) {
            return failed("SIGTERM", error);
        }
        match wait_until_terminal(self.pgid, Some(grace)).await {
            Ok(()) => self.reap(KillOutcome::Terminated, grace).await,
            Err(error) if error.kind() == io::ErrorKind::TimedOut => {
                self.kill_residual_and_reap(KillOutcome::Killed, grace)
                    .await
            }
            Err(error) => failed("grace observation", error),
        }
    }

    pub(super) fn kill_on_drop(&mut self) {
        let _ = signal_group(self.pgid, libc::SIGKILL);
        let _ = self.child.start_kill();
    }

    async fn wait_for_root_exit(&self) -> io::Result<()> {
        loop {
            if root_exited(self.pgid)? {
                return Ok(());
            }
            tokio::time::sleep(WAIT_POLL_INTERVAL).await;
        }
    }

    async fn kill_residual_and_reap(
        &mut self,
        outcome: KillOutcome,
        grace: Duration,
    ) -> Termination {
        let outcome = match has_live(self.pgid).await {
            Ok(true) => {
                if let Err(error) = signal_group(self.pgid, libc::SIGKILL) {
                    return failed("SIGKILL", error);
                }
                if let Err(error) = wait_until_terminal(self.pgid, Some(grace)).await {
                    return failed("terminal proof", error);
                }
                KillOutcome::Killed
            }
            Ok(false) => outcome,
            Err(error) => return failed("terminal inspection", error),
        };
        self.reap(outcome, grace).await
    }

    async fn reap(&mut self, outcome: KillOutcome, grace: Duration) -> Termination {
        match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(Ok(status)) => completed(outcome, status),
            Ok(Err(error)) => failed("terminal wait", error),
            Err(_) => failed(
                "terminal wait",
                io::Error::new(io::ErrorKind::TimedOut, "terminal proof timed out"),
            ),
        }
    }
}

fn completed(outcome: KillOutcome, status: ExitStatus) -> Termination {
    Termination {
        outcome,
        exit_code: status.code(),
    }
}

fn failed(stage: &str, error: io::Error) -> Termination {
    Termination {
        outcome: KillOutcome::KillFailed(format!("process group {stage} failed: {error}")),
        exit_code: None,
    }
}

fn signal_group(pgid: i32, signal: i32) -> io::Result<()> {
    if unsafe { libc::killpg(pgid, signal) } == 0 {
        return Ok(());
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error)
    }
}

fn root_exited(pid: i32) -> io::Result<bool> {
    let mut info = std::mem::MaybeUninit::<libc::siginfo_t>::zeroed();
    let options = libc::WEXITED | libc::WNOHANG | libc::WNOWAIT;
    let result =
        unsafe { libc::waitid(libc::P_PID, pid as libc::id_t, info.as_mut_ptr(), options) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    let info = unsafe { info.assume_init() };
    Ok(unsafe { info.si_pid() } == pid)
}

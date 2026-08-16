use std::io;
use std::process::ExitStatus;
use std::time::Duration;

use tokio::process::{ChildStderr, ChildStdin, ChildStdout, Command};

#[cfg(unix)]
mod unix;
#[cfg(unix)]
mod unix_members;
#[cfg(windows)]
mod windows;

#[cfg(unix)]
use self::unix::PlatformChild;
#[cfg(windows)]
use self::windows::PlatformChild;

#[derive(Debug)]
pub struct SpawnedChild {
    child: PlatformChild,
    armed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KillOutcome {
    Terminated,
    Killed,
    KillFailed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Termination {
    pub outcome: KillOutcome,
    pub exit_code: Option<i32>,
}

impl SpawnedChild {
    pub fn spawn(command: Command) -> io::Result<Self> {
        Ok(Self {
            child: PlatformChild::spawn(command)?,
            armed: true,
        })
    }

    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.take_stdin()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.take_stdout()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.take_stderr()
    }

    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        let status = self.child.wait().await?;
        self.armed = false;
        Ok(status)
    }

    pub async fn terminate(&mut self, grace: Duration) -> Termination {
        if !self.armed {
            return Termination {
                outcome: KillOutcome::Terminated,
                exit_code: None,
            };
        }
        let termination = self.child.terminate(grace).await;
        if !matches!(termination.outcome, KillOutcome::KillFailed(_)) {
            self.armed = false;
        }
        termination
    }
}

impl Drop for SpawnedChild {
    fn drop(&mut self) {
        if self.armed {
            self.child.kill_on_drop();
        }
    }
}

use std::io;
use std::process::ExitStatus;
use std::time::Duration;

use process_wrap::tokio::{ChildWrapper, CommandWrap, JobObject, KillOnDrop};
use tokio::process::{ChildStderr, ChildStdin, ChildStdout, Command};

use super::{KillOutcome, Termination};

#[derive(Debug)]
pub(super) struct PlatformChild {
    child: Box<dyn ChildWrapper>,
}

impl PlatformChild {
    pub(super) fn spawn(command: Command) -> io::Result<Self> {
        let mut command = CommandWrap::from(command);
        command.wrap(KillOnDrop).wrap(JobObject);
        Ok(Self {
            child: command.spawn()?,
        })
    }

    pub(super) fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout().take()
    }

    pub(super) fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin().take()
    }

    pub(super) fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr().take()
    }

    pub(super) async fn wait(&mut self) -> io::Result<ExitStatus> {
        self.child.wait().await
    }

    pub(super) async fn terminate(&mut self, grace: Duration) -> Termination {
        if let Err(error) = self.child.start_kill() {
            return failed("termination", error);
        }
        match tokio::time::timeout(grace, self.child.wait()).await {
            Ok(Ok(status)) => Termination {
                outcome: KillOutcome::Killed,
                exit_code: status.code(),
            },
            Ok(Err(error)) => failed("terminal wait", error),
            Err(_) => failed(
                "terminal wait",
                io::Error::new(io::ErrorKind::TimedOut, "terminal proof timed out"),
            ),
        }
    }

    pub(super) fn kill_on_drop(&mut self) {
        let _ = self.child.start_kill();
    }
}

fn failed(stage: &str, error: io::Error) -> Termination {
    Termination {
        outcome: KillOutcome::KillFailed(format!("process job {stage} failed: {error}")),
        exit_code: None,
    }
}

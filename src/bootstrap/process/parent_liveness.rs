use std::time::Duration;

const PARENT_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Reject configuration mistakes before starting the expensive Hub bootstrap.
pub fn validate(parent_pid: u32) -> anyhow::Result<()> {
    validate_with(parent_pid, std::process::id(), super::discovery::is_alive)
}

fn validate_with(
    parent_pid: u32,
    current_pid: u32,
    is_alive: impl FnOnce(u32) -> bool,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        parent_pid != 0,
        "desktop parent pid must be greater than zero"
    );
    anyhow::ensure!(
        parent_pid != current_pid,
        "desktop parent pid {parent_pid} refers to the Loopal process itself"
    );
    anyhow::ensure!(
        is_alive(parent_pid),
        "desktop parent process {parent_pid} is not running"
    );
    Ok(())
}

/// Resolves once the supervising Desktop process no longer exists.
pub async fn wait_until_exit(parent_pid: u32) {
    wait_until_exit_with(parent_pid, PARENT_POLL_INTERVAL, super::discovery::is_alive).await;
}

async fn wait_until_exit_with(
    parent_pid: u32,
    interval: Duration,
    mut is_alive: impl FnMut(u32) -> bool,
) {
    loop {
        if !is_alive(parent_pid) {
            return;
        }
        tokio::time::sleep(interval).await;
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn validate_accepts_a_distinct_live_parent() {
        assert!(validate_with(41, 42, |pid| pid == 41).is_ok());
    }

    #[test]
    fn validate_rejects_the_current_process() {
        let error = validate_with(42, 42, |_| true).unwrap_err();
        assert!(error.to_string().contains("itself"));
    }

    #[test]
    fn validate_rejects_a_missing_parent() {
        let error = validate_with(41, 42, |_| false).unwrap_err();
        assert!(error.to_string().contains("is not running"));
    }

    #[tokio::test]
    async fn wait_polling_resolves_after_parent_disappears() {
        let calls = Cell::new(0);
        wait_until_exit_with(41, Duration::from_millis(1), |_| {
            let call = calls.get() + 1;
            calls.set(call);
            call < 3
        })
        .await;
        assert_eq!(calls.get(), 3);
    }

    #[tokio::test]
    async fn wait_polling_returns_immediately_for_already_dead_parent() {
        let calls = Cell::new(0);
        wait_until_exit_with(41, Duration::from_secs(60), |_| {
            calls.set(calls.get() + 1);
            false
        })
        .await;
        assert_eq!(calls.get(), 1);
    }
}

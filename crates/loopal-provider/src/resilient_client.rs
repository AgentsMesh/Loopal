use reqwest::Client;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::time::Duration;

/// HTTP client wrapper that automatically rebuilds the underlying `reqwest::Client`
/// when consecutive network errors indicate connection pool corruption.
///
/// Network disruptions (proxy switches, interface changes) can poison the connection
/// pool inside a long-lived `reqwest::Client`, causing all subsequent requests to fail
/// instantly even after the network recovers. `ResilientClient` detects this pattern
/// and recreates the client to restore connectivity.
///
/// Each `get()` call returns the current client and a generation token. The token
/// prevents stale requests (from a pre-rebuild client) from resetting the error
/// counter after a rebuild has already occurred.
pub(crate) struct ResilientClient {
    client: Mutex<Client>,
    timeout: Duration,
    connect_timeout: Duration,
    consecutive_errors: AtomicU32,
    /// Monotonically increasing generation, bumped on each rebuild.
    generation: AtomicU64,
}

/// Number of consecutive network errors before rebuilding the client.
const REBUILD_THRESHOLD: u32 = 2;

impl ResilientClient {
    pub fn new(timeout: Duration, connect_timeout: Duration) -> Self {
        let client = build_client(timeout, connect_timeout);
        Self {
            client: Mutex::new(client),
            timeout,
            connect_timeout,
            consecutive_errors: AtomicU32::new(0),
            generation: AtomicU64::new(0),
        }
    }

    /// Clone the current client and return it with the current generation token.
    /// `reqwest::Client` is backed by an `Arc`, so cloning is near-free.
    pub fn get(&self) -> (Client, u64) {
        let guard = self.client.lock().expect("client mutex poisoned");
        let generation = self.generation.load(Ordering::Relaxed);
        (guard.clone(), generation)
    }

    /// Reset the consecutive error counter after a successful request.
    /// Ignored if a rebuild occurred since this request's `get()` call.
    pub fn report_success(&self, generation: u64) {
        if self.generation.load(Ordering::Relaxed) == generation {
            self.consecutive_errors.store(0, Ordering::Relaxed);
        }
    }

    /// Increment the consecutive error counter. If the threshold is reached,
    /// rebuild the client under the mutex to guarantee exactly-once replacement.
    /// Ignored if a rebuild occurred since this request's `get()` call.
    pub fn report_network_error(&self, generation: u64) {
        if self.generation.load(Ordering::Relaxed) != generation {
            return; // Client already rebuilt; this error is from a stale pool.
        }
        let count = self.consecutive_errors.fetch_add(1, Ordering::Relaxed) + 1;
        if count >= REBUILD_THRESHOLD {
            let mut guard = self.client.lock().expect("client mutex poisoned");
            // Double-check under lock — another thread may have already rebuilt.
            if self.generation.load(Ordering::Relaxed) == generation {
                *guard = build_client(self.timeout, self.connect_timeout);
                self.generation.fetch_add(1, Ordering::Relaxed);
                self.consecutive_errors.store(0, Ordering::Relaxed);
                tracing::info!(
                    threshold = REBUILD_THRESHOLD,
                    "rebuilt HTTP client after consecutive network errors"
                );
            }
        }
    }
}

fn build_client(timeout: Duration, connect_timeout: Duration) -> Client {
    Client::builder()
        .timeout(timeout)
        .connect_timeout(connect_timeout)
        .pool_idle_timeout(Duration::from_secs(60))
        .tcp_keepalive(Duration::from_secs(30))
        .build()
        .expect("failed to build HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rebuild_after_threshold() {
        let rc = ResilientClient::new(Duration::from_secs(30), Duration::from_secs(5));
        let (_, gen0) = rc.get();

        // First error: below threshold, no rebuild.
        rc.report_network_error(gen0);
        assert_eq!(rc.generation.load(Ordering::Relaxed), 0);

        // Second error: reaches threshold, triggers rebuild.
        rc.report_network_error(gen0);
        assert_eq!(rc.generation.load(Ordering::Relaxed), 1);
        assert_eq!(rc.consecutive_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn success_resets_counter() {
        let rc = ResilientClient::new(Duration::from_secs(30), Duration::from_secs(5));
        let (_, g) = rc.get();

        rc.report_network_error(g);
        assert_eq!(rc.consecutive_errors.load(Ordering::Relaxed), 1);

        rc.report_success(g);
        assert_eq!(rc.consecutive_errors.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn stale_generation_ignored() {
        let rc = ResilientClient::new(Duration::from_secs(30), Duration::from_secs(5));
        let (_, gen0) = rc.get();

        // Force a rebuild.
        rc.report_network_error(gen0);
        rc.report_network_error(gen0);
        assert_eq!(rc.generation.load(Ordering::Relaxed), 1);

        // Stale success from gen0 must not reset the counter.
        rc.report_network_error(rc.generation.load(Ordering::Relaxed));
        rc.report_success(gen0); // stale — should be ignored
        assert_eq!(rc.consecutive_errors.load(Ordering::Relaxed), 1);

        // Stale error from gen0 must not increment or rebuild.
        rc.report_network_error(gen0); // stale — should be ignored
        assert_eq!(rc.consecutive_errors.load(Ordering::Relaxed), 1);
        assert_eq!(rc.generation.load(Ordering::Relaxed), 1);
    }
}

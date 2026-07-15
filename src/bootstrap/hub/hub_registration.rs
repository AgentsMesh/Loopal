use chrono::Utc;
use tracing::error;

use super::{discovery, token_channel};

pub struct HubRegistration {
    pid: u32,
    token_channel: Option<tokio::task::JoinHandle<()>>,
    withdrawn: bool,
}

impl HubRegistration {
    pub fn register(
        pid: u32,
        token: &str,
        addr: &str,
        cwd: &std::path::Path,
        session_id: &str,
    ) -> Self {
        let token_channel = match token_channel::bind_token_channel(pid, token.to_string()) {
            Ok(handle) => Some(handle),
            Err(error) => {
                error!(%error, "failed to bind hub token channel; discovery unavailable");
                None
            }
        };
        if token_channel.is_some() {
            let record = discovery::HubDiscoveryRecord {
                pid,
                tcp_addr: addr.to_string(),
                cwd: cwd.display().to_string(),
                started_at: Utc::now().to_rfc3339(),
                root_session_id: session_id.to_string(),
            };
            if let Err(error) = discovery::write_record(&record) {
                error!(%error, "failed to write hub discovery record");
            }
        }
        Self {
            pid,
            token_channel,
            withdrawn: false,
        }
    }

    pub fn withdraw(&mut self) {
        if self.withdrawn {
            return;
        }
        self.withdrawn = true;
        discovery::remove_record(self.pid);
        if let Some(channel) = self.token_channel.take() {
            channel.abort();
        }
        token_channel::cleanup_channel(self.pid);
    }
}

impl Drop for HubRegistration {
    fn drop(&mut self) {
        self.withdraw();
    }
}

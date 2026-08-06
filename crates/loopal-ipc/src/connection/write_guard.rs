use std::sync::Arc;
use std::time::Duration;

use tracing::warn;

use crate::transport::Transport;

pub(super) struct FrameWriteGuard {
    transport: Option<Arc<dyn Transport>>,
}

impl FrameWriteGuard {
    pub(super) fn new(transport: Arc<dyn Transport>) -> Self {
        Self {
            transport: Some(transport),
        }
    }

    pub(super) fn disarm(mut self) {
        self.transport = None;
    }
}

impl Drop for FrameWriteGuard {
    fn drop(&mut self) {
        let Some(transport) = self.transport.take() else {
            return;
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        runtime.spawn(async move {
            if tokio::time::timeout(Duration::from_secs(2), transport.close())
                .await
                .is_err()
            {
                warn!("timed out closing IPC transport after cancelled frame write");
            }
        });
    }
}

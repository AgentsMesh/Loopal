use std::sync::Arc;

use tokio::sync::Mutex;

use crate::{Hub, HubUplink};

pub(super) struct ForwardingClaim {
    hub: Arc<Mutex<Hub>>,
    key: (String, String),
    uplink: Arc<HubUplink>,
    armed: bool,
}

impl ForwardingClaim {
    pub(super) fn new(hub: Arc<Mutex<Hub>>, key: (String, String), uplink: Arc<HubUplink>) -> Self {
        Self {
            hub,
            key,
            uplink,
            armed: true,
        }
    }

    pub(super) async fn release(&mut self) {
        release_record(&self.hub, &self.key, &self.uplink).await;
        self.armed = false;
    }

    pub(super) fn commit(&mut self) {
        self.armed = false;
    }
}

impl Drop for ForwardingClaim {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let hub = self.hub.clone();
        let key = self.key.clone();
        let uplink = self.uplink.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                release_record(&hub, &key, &uplink).await;
            });
        }
    }
}

pub(super) async fn claim_record(
    hub: &Arc<Mutex<Hub>>,
    key: &(String, String),
) -> Result<Option<crate::pending_relay::PendingRemoteQuestionInfo>, String> {
    let mut h = hub.lock().await;
    let active = h.uplink.clone();
    let Some(record) = h.pending_remote_questions.get_mut(key) else {
        return Ok(None);
    };
    if !active
        .as_ref()
        .is_some_and(|uplink| Arc::ptr_eq(uplink, &record.uplink))
    {
        return Err("remote question belongs to a stale uplink generation".into());
    }
    if record.forwarding {
        return Ok(None);
    }
    record.forwarding = true;
    Ok(Some(record.clone()))
}

async fn release_record(hub: &Arc<Mutex<Hub>>, key: &(String, String), uplink: &Arc<HubUplink>) {
    let mut h = hub.lock().await;
    if let Some(current) = h.pending_remote_questions.get_mut(key)
        && Arc::ptr_eq(&current.uplink, uplink)
    {
        current.forwarding = false;
    }
}

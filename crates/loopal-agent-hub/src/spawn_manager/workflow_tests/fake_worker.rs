use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use tokio::sync::Notify;

use crate::spawn_manager::spawn::SpawnProcess;

#[derive(Default)]
pub(super) struct Probe {
    pub(super) starts: AtomicUsize,
    pub(super) interrupts: AtomicUsize,
    pub(super) shutdowns: AtomicUsize,
    pub(super) process_shutdowns: AtomicUsize,
    pub(super) process_stopped: AtomicBool,
    pub(super) block_process_shutdown: AtomicBool,
    pub(super) fail_process_shutdown: AtomicBool,
    pub(super) reply_to_shutdown: AtomicBool,
    pub(super) process_shutdown_release: Notify,
}

pub(super) struct FakeProcess {
    transport: Arc<dyn Transport>,
    probe: Arc<Probe>,
}

impl FakeProcess {
    pub(super) fn new(transport: Arc<dyn Transport>, probe: Arc<Probe>) -> Self {
        Self { transport, probe }
    }
}

impl SpawnProcess for FakeProcess {
    fn transport(&self) -> Arc<dyn Transport> {
        self.transport.clone()
    }

    fn shutdown(self) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
        Box::pin(async move {
            self.probe.process_shutdowns.fetch_add(1, Ordering::SeqCst);
            if self.probe.block_process_shutdown.load(Ordering::SeqCst) {
                self.probe.process_shutdown_release.notified().await;
            }
            if self.probe.fail_process_shutdown.load(Ordering::SeqCst) {
                anyhow::bail!("injected process shutdown failure");
            }
            self.probe.process_stopped.store(true, Ordering::SeqCst);
            Ok(())
        })
    }

    fn wait(self) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send>> {
        Box::pin(async { Ok(()) })
    }
}

pub(super) fn spawn_peer(transport: Arc<dyn Transport>, probe: Arc<Probe>) {
    let (peer, mut incoming) = Connection::new(transport).into_listening();
    tokio::spawn(async move {
        while let Some(message) = incoming.recv().await {
            match message {
                Incoming::Request { id, method, .. } if method == methods::INITIALIZE.name => {
                    peer.respond(id, serde_json::json!({"protocol_version": 1}))
                        .await
                        .unwrap();
                }
                Incoming::Request { id, method, params } if method == methods::AGENT_START.name => {
                    probe.starts.fetch_add(1, Ordering::SeqCst);
                    peer.respond(id, serde_json::json!({"session_id": params["session_id"]}))
                        .await
                        .unwrap();
                }
                Incoming::Request { id, method, .. } if method == methods::AGENT_SHUTDOWN.name => {
                    probe.shutdowns.fetch_add(1, Ordering::SeqCst);
                    if probe.reply_to_shutdown.load(Ordering::SeqCst) {
                        peer.respond(id, serde_json::json!({})).await.unwrap();
                    }
                }
                Incoming::Notification { method, .. }
                    if method == methods::AGENT_INTERRUPT.name =>
                {
                    probe.interrupts.fetch_add(1, Ordering::SeqCst);
                }
                _ => {}
            }
        }
    });
}

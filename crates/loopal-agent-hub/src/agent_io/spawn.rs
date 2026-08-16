use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use tokio::sync::Mutex;
use tracing::info;

use crate::finish::finish_and_deliver_exact;
use crate::hub::Hub;
use crate::types::AgentExecutionRef;

use super::agent_io_loop_exact;

pub fn spawn_io_loop(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
) {
    let name = name.to_string();
    tokio::spawn(async move {
        let execution = hub.lock().await.registry.current_execution(&name);
        let Some(execution) = execution else {
            tracing::warn!(agent = %name, "spawn_io_loop requires a registered execution");
            return;
        };
        let completion = agent_io_loop_exact(
            hub.clone(),
            dispatcher,
            conn.clone(),
            rx,
            name.clone(),
            execution.clone(),
        )
        .await;
        finish_and_deliver_exact(&hub, &name, completion, &conn, &execution).await;
        info!(agent = %name, "agent IO loop ended");
    });
}

pub(crate) fn spawn_io_loop_exact(
    hub: Arc<Mutex<Hub>>,
    dispatcher: Arc<loopal_ipc::Dispatcher>,
    name: &str,
    conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    execution: AgentExecutionRef,
) {
    let io_hub = hub.clone();
    let io_name = name.to_string();
    let finish_name = name.to_string();
    tokio::spawn(async move {
        let completion = agent_io_loop_exact(
            io_hub,
            dispatcher,
            conn.clone(),
            rx,
            io_name,
            execution.clone(),
        )
        .await;
        finish_and_deliver_exact(&hub, &finish_name, completion, &conn, &execution).await;
        info!(agent = %finish_name, "agent IO loop ended");
    });
}

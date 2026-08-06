use std::sync::Arc;

use loopal_ipc::connection::{Connection, Incoming, Listening};
use tokio::sync::{Mutex, mpsc};

use super::register::RegisterResult;
use crate::Hub;

pub(super) async fn reserve_ack_and_start(
    hub: Arc<Mutex<Hub>>,
    conn: Arc<Connection<Listening>>,
    incoming: mpsc::Receiver<Incoming>,
    result: RegisterResult,
) -> Result<(), String> {
    let reservation = hub
        .lock()
        .await
        .registry
        .reserve_connection(&result.name, conn.clone());
    if let Err(error) = reservation {
        reject_registration(&conn, result.request_id, &error).await;
        super::close_bounded(&conn).await;
        return Err(error);
    }

    let mut guard = ReservationGuard::new(hub.clone(), result.name.clone(), conn.clone());
    if let Err(error) = super::acknowledge_register(&conn, &result).await {
        guard.cancel().await;
        return Err(error);
    }

    let dispatcher = Arc::new(crate::dispatch::build_hub_dispatcher(hub.clone()));
    if let Err(error) = crate::agent_io::start_reserved_agent_io(
        hub,
        dispatcher,
        result.name,
        conn.clone(),
        incoming,
    )
    .await
    {
        guard.cancel().await;
        super::close_bounded(&conn).await;
        return Err(error);
    }
    guard.disarm();
    Ok(())
}

async fn reject_registration(conn: &Connection<Listening>, id: i64, error: &str) {
    let response = conn.respond_error(id, loopal_ipc::jsonrpc::INVALID_REQUEST, error);
    let _ = tokio::time::timeout(super::REGISTER_ACK_DEADLINE, response).await;
}

struct ReservationGuard {
    hub: Arc<Mutex<Hub>>,
    name: String,
    conn: Arc<Connection<Listening>>,
    armed: bool,
}

impl ReservationGuard {
    fn new(hub: Arc<Mutex<Hub>>, name: String, conn: Arc<Connection<Listening>>) -> Self {
        Self {
            hub,
            name,
            conn,
            armed: true,
        }
    }

    async fn cancel(&mut self) {
        if self.armed {
            self.hub
                .lock()
                .await
                .registry
                .cancel_connection_reservation(&self.name, &self.conn);
            self.armed = false;
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ReservationGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let hub = self.hub.clone();
        let name = self.name.clone();
        let conn = self.conn.clone();
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            runtime.spawn(async move {
                hub.lock()
                    .await
                    .registry
                    .cancel_connection_reservation(&name, &conn);
                let _ = tokio::time::timeout(super::TRANSPORT_CLOSE_DEADLINE, conn.close()).await;
            });
        }
    }
}

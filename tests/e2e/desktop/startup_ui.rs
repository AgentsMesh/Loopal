use std::sync::Arc;

use loopal_ipc::TcpTransport;
use loopal_ipc::connection::{Connection, Listening};
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use tokio::net::TcpStream;

pub async fn register(addr: &str, token: &str) -> std::io::Result<Arc<Connection<Listening>>> {
    let stream = TcpStream::connect(addr).await?;
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (connection, mut incoming) = Connection::new(transport).into_listening();
    let registered = connection
        .send_request(
            methods::HUB_REGISTER.name,
            serde_json::json!({
                "name": "desktop-startup-e2e",
                "token": token,
                "role": "ui_client",
                "capabilities": {
                    "permission": true,
                    "question": true,
                    "plan_approval": true,
                },
            }),
        )
        .await
        .map_err(std::io::Error::other)?;
    if registered["ok"] != true {
        return Err(std::io::Error::other(format!(
            "startup UI registration rejected: {registered}"
        )));
    }
    tokio::spawn(async move { while incoming.recv().await.is_some() {} });
    Ok(connection)
}

use std::sync::Arc;

use loopal_agent_hub::Hub;
use loopal_agent_hub::dispatch::dispatch_hub_request;
use loopal_ipc::Connection;
use loopal_ipc::connection::Incoming;
use tokio::sync::Mutex;

pub const ED25519_UNENCRYPTED: &str = "-----BEGIN OPENSSH PRIVATE KEY-----
b3BlbnNzaC1rZXktdjEAAAAABG5vbmUAAAAEbm9uZQAAAAAAAAABAAAAMwAAAAtzc2gtZW
QyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQAAAJCfEwtqnxML
agAAAAtzc2gtZWQyNTUxOQAAACB7Ci6nqZYaVvrjm8+XbzII89TsXzP111AflR7WeorBjQ
AAAEADBJvjZT8X6JRJI8xVq/1aU8nMVgOtVnmdwqWwrSlXG3sKLqeplhpW+uObz5dvMgjz
1OxfM/XXUB+VHtZ6isGNAAAADHN0cjRkQGNhcmJvbgE=
-----END OPENSSH PRIVATE KEY-----
";

pub const PUBKEY_ALICE: &str =
    "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIHsKLqeplhpW+uObz5dvMgjz1OxfM/XXUB+VHtZ6isGN alice@rust";

#[cfg(unix)]
pub fn write_key_0600(path: &std::path::Path, content: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, content).unwrap();
    let mut p = std::fs::metadata(path).unwrap().permissions();
    p.set_mode(0o600);
    std::fs::set_permissions(path, p).unwrap();
}

#[cfg(not(unix))]
pub fn write_key_0600(path: &std::path::Path, content: &str) {
    std::fs::write(path, content).unwrap();
}

pub fn spawn_hub_dispatch_loop(
    hub: Arc<Mutex<Hub>>,
    hub_conn: Arc<Connection>,
    from_agent: String,
) {
    let mut rx = hub_conn.start();
    let conn = hub_conn.clone();
    tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if let Incoming::Request { id, method, params } = msg {
                let outcome =
                    dispatch_hub_request(&hub, &method, params, from_agent.clone()).await;
                match outcome {
                    Ok(v) => {
                        let _ = conn.respond(id, v).await;
                    }
                    Err(m) => {
                        let _ = conn.respond_error(id, -32000, &m).await;
                    }
                }
            }
        }
    });
}

use std::sync::Arc;

use loopal_agent_hub::Hub;
use loopal_ipc::connection::Incoming;
use loopal_ipc::{Connection, Listening};
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

pub async fn spawn_hub_dispatch_loop(
    hub: Arc<Mutex<Hub>>,
    hub_conn: Arc<Connection<Listening>>,
    rx: tokio::sync::mpsc::Receiver<Incoming>,
    from_agent: String,
) {
    let dispatcher = Arc::new(loopal_agent_hub::dispatch::build_hub_dispatcher(
        hub.clone(),
    ));
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    loopal_agent_hub::agent_io::start_agent_io(
        hub,
        dispatcher,
        &from_agent,
        hub_conn,
        rx,
        Some(ready_tx),
    );
    ready_rx.await.expect("Agent fixture must register");
}

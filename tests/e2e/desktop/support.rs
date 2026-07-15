use std::io::Write as _;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;

use loopal_ipc::connection::Connection;
use loopal_ipc::protocol::methods;
use loopal_ipc::transport::Transport;
use loopal_ipc::{DESKTOP_PROTOCOL_VERSION, DesktopHandshake, DesktopHandshakeEvent, TcpTransport};
use tokio::io::{AsyncBufReadExt as _, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::timeout;

pub const STARTUP_DEADLINE: Duration = Duration::from_secs(20);
pub const EXIT_DEADLINE: Duration = Duration::from_secs(8);

fn binary_path() -> std::path::PathBuf {
    let configured = std::path::PathBuf::from(
        std::env::var("LOOPAL_BINARY").expect("LOOPAL_BINARY env required"),
    );
    if configured.is_absolute() {
        configured
    } else {
        std::env::current_dir()
            .expect("test current directory")
            .join(configured)
    }
}

pub fn write_mock_fixture() -> tempfile::NamedTempFile {
    let mut file = tempfile::NamedTempFile::new().expect("create mock provider fixture");
    file.write_all(br#"[[{"type":"text","text":"ok"},{"type":"usage"},{"type":"done"}]]"#)
        .expect("write fixture");
    file.flush().expect("flush fixture");
    file
}

pub fn spawn_desktop(
    home: &tempfile::TempDir,
    fixture: &tempfile::NamedTempFile,
    parent_pid: u32,
) -> Child {
    spawn_desktop_config(home, fixture, parent_pid, None, None)
}

pub fn spawn_desktop_with_resume(
    home: &tempfile::TempDir,
    fixture: &tempfile::NamedTempFile,
    parent_pid: u32,
    resume: Option<&str>,
) -> Child {
    spawn_desktop_config(home, fixture, parent_pid, resume, None)
}

pub fn spawn_desktop_with_root_timeout(
    home: &tempfile::TempDir,
    fixture: &tempfile::NamedTempFile,
    parent_pid: u32,
) -> Child {
    spawn_desktop_config(home, fixture, parent_pid, None, Some(0))
}

fn spawn_desktop_config(
    home: &tempfile::TempDir,
    fixture: &tempfile::NamedTempFile,
    parent_pid: u32,
    resume: Option<&str>,
    root_timeout_ms: Option<u64>,
) -> Child {
    let mut command = Command::new(binary_path());
    command.args(["desktop", "serve", "--parent-pid", &parent_pid.to_string()]);
    if let Some(session_id) = resume {
        command.args(["--resume", session_id]);
    }
    if let Some(timeout) = root_timeout_ms {
        command.env("LOOPAL_TEST_ROOT_VIEW_TIMEOUT_MS", timeout.to_string());
    }
    command
        .current_dir(home.path())
        .env("HOME", home.path())
        .env("LOOPAL_TEST_PROVIDER", fixture.path())
        .env("LOOPAL_TEST_SESSION_DIR", home.path().join("sessions"))
        .env("LOOPAL_MCP_STARTUP_WAIT_SECS", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn loopal desktop serve")
}

pub fn assert_common_metadata(handshake: &DesktopHandshake, pid: u32, parent_pid: u32) {
    assert_eq!(handshake.protocol_version, DESKTOP_PROTOCOL_VERSION);
    assert!(
        !handshake.server_version.is_empty(),
        "server version must be stamped"
    );
    assert_eq!(handshake.pid, pid);
    assert_eq!(handshake.parent_pid, Some(parent_pid));
}

pub async fn read_alive_and_ready(
    stdout: tokio::process::ChildStdout,
) -> std::io::Result<(DesktopHandshake, DesktopHandshake, Vec<String>)> {
    let mut reader = BufReader::new(stdout);
    let mut alive = None;
    let mut ready = None;
    let mut observed = Vec::new();

    for _ in 0..64 {
        let mut line = String::new();
        let read = reader.read_line(&mut line).await?;
        if read == 0 {
            return Err(std::io::Error::new(
                std::io::ErrorKind::UnexpectedEof,
                "stdout closed before alive + ready",
            ));
        }
        observed.push(line.trim_end().to_string());
        let Some(handshake) = DesktopHandshake::parse(&line).map_err(std::io::Error::other)? else {
            continue;
        };
        match &handshake.event {
            DesktopHandshakeEvent::Alive { .. } => alive = Some(handshake.clone()),
            DesktopHandshakeEvent::SessionCreated { .. } => {}
            DesktopHandshakeEvent::Ready { .. } => ready = Some(handshake.clone()),
            DesktopHandshakeEvent::Error { code, message } => {
                return Err(std::io::Error::other(format!(
                    "Desktop startup failed: {code}: {message}"
                )));
            }
        }
        if alive.is_some() && ready.is_some() {
            return Ok((alive.take().unwrap(), ready.take().unwrap(), observed));
        }
    }
    Err(std::io::Error::other(
        "Desktop wrote 64 lines without alive + ready",
    ))
}

pub async fn request_hub_shutdown(addr: &str, token: &str) {
    let stream = TcpStream::connect(addr).await.expect("connect Desktop Hub");
    let transport: Arc<dyn Transport> = Arc::new(TcpTransport::new(stream));
    let (connection, _incoming) = Connection::new(transport).into_listening();
    let registered = connection
        .send_request(
            methods::HUB_REGISTER.name,
            serde_json::json!({"name": "desktop-e2e", "token": token, "role": "ui_client"}),
        )
        .await
        .expect("register Desktop E2E client");
    assert_eq!(registered["ok"], true);
    connection
        .send_request(methods::HUB_SHUTDOWN.name, serde_json::json!({}))
        .await
        .expect("request Desktop Hub shutdown");
}

pub async fn wait_for_registration_withdrawal(home: &tempfile::TempDir, pid: u32) {
    let run_dir = home.path().join(".loopal").join("run");
    timeout(Duration::from_secs(2), async {
        while run_dir.join(format!("{pid}.json")).exists()
            || run_dir.join(format!("{pid}.sock")).exists()
        {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("Desktop registration must be withdrawn before agent drain completes");
}

use loopal_ipc::{DesktopHandshake, HandshakeLine};
use tokio::io::AsyncWriteExt as _;
use tracing::error;

#[derive(Debug, Clone, Copy)]
pub(super) enum StartupProtocol {
    HubOnly,
    Desktop { parent_pid: Option<u32> },
}

impl StartupProtocol {
    pub(super) async fn write_alive(self, addr: String, token: String) {
        match self {
            Self::HubOnly => write_line(&HandshakeLine::Alive { addr, token }).await,
            Self::Desktop { parent_pid } => {
                write_raw(
                    DesktopHandshake::alive(
                        env!("LOOPAL_VERSION"),
                        std::process::id(),
                        parent_pid,
                        addr,
                        token,
                    )
                    .encode(),
                )
                .await;
            }
        }
    }

    pub(super) async fn write_ready(self, addr: &str, token: &str, session_id: &str) {
        match self {
            Self::HubOnly => {
                write_line(&HandshakeLine::Ready {
                    session_id: session_id.to_string(),
                })
                .await;
                write_line(&HandshakeLine::Legacy {
                    addr: addr.to_string(),
                    token: token.to_string(),
                    session_id: session_id.to_string(),
                })
                .await;
            }
            Self::Desktop { parent_pid } => {
                write_raw(
                    DesktopHandshake::ready(
                        env!("LOOPAL_VERSION"),
                        std::process::id(),
                        parent_pid,
                        session_id,
                    )
                    .encode(),
                )
                .await;
            }
        }
    }

    pub(super) async fn write_session_created(self, session_id: &str) {
        if let Self::Desktop { parent_pid } = self {
            write_raw(
                DesktopHandshake::session_created(
                    env!("LOOPAL_VERSION"),
                    std::process::id(),
                    parent_pid,
                    session_id,
                )
                .encode(),
            )
            .await;
        }
    }

    pub(super) async fn write_error(self, code: &str, message: impl Into<String>) {
        let message = message.into();
        match self {
            Self::HubOnly => write_line(&HandshakeLine::Error(message)).await,
            Self::Desktop { parent_pid } => {
                write_raw(
                    DesktopHandshake::error(
                        env!("LOOPAL_VERSION"),
                        std::process::id(),
                        parent_pid,
                        code,
                        message,
                    )
                    .encode(),
                )
                .await;
            }
        }
    }
}

pub(super) async fn write_desktop_error(
    parent_pid: Option<u32>,
    code: &str,
    message: impl Into<String>,
) {
    StartupProtocol::Desktop { parent_pid }
        .write_error(code, message)
        .await;
}

async fn write_line(line: &HandshakeLine) {
    write_raw(line.encode()).await;
}

async fn write_raw(encoded: String) {
    let mut out = tokio::io::stdout();
    if let Err(e) = out.write_all(encoded.as_bytes()).await {
        error!(error = %e, "failed to write handshake line");
    }
    let _ = out.flush().await;
}

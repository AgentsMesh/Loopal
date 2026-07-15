//! Versioned startup handshake for `loopal desktop serve`.
//!
//! Core handshake records use `LOOPAL_DESKTOP `. Optional lifecycle events
//! use a separate prefix so older strict Desktop parsers can ignore them.

use serde::{Deserialize, Serialize};

pub const DESKTOP_HANDSHAKE_PREFIX: &str = "LOOPAL_DESKTOP ";
pub const DESKTOP_EVENT_PREFIX: &str = "LOOPAL_DESKTOP_EVENT ";
pub const DESKTOP_PROTOCOL_VERSION: u32 = 1;
pub const DESKTOP_TRANSPORT: &str = "tcp_jsonrpc_ndjson";
pub const DESKTOP_CAPABILITY_HUB_UI: &str = "hub_ui_v1";
pub const DESKTOP_CAPABILITY_WORKSPACE: &str = "workspace_v1";

/// A single machine-readable line emitted while the Desktop Host starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopHandshake {
    pub protocol_version: u32,
    pub server_version: String,
    pub pid: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_pid: Option<u32>,
    #[serde(flatten)]
    pub event: DesktopHandshakeEvent,
}

/// Alive lets Desktop attach before the root Agent can request approval;
/// SessionCreated exposes the durable identity before full readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "phase", rename_all = "snake_case")]
pub enum DesktopHandshakeEvent {
    Alive {
        addr: String,
        token: String,
        transport: String,
        capabilities: Vec<String>,
    },
    Ready {
        session_id: String,
    },
    SessionCreated {
        session_id: String,
    },
    Error {
        code: String,
        message: String,
    },
}

impl DesktopHandshake {
    pub fn alive(
        server_version: impl Into<String>,
        pid: u32,
        parent_pid: Option<u32>,
        addr: impl Into<String>,
        token: impl Into<String>,
    ) -> Self {
        Self {
            protocol_version: DESKTOP_PROTOCOL_VERSION,
            server_version: server_version.into(),
            pid,
            parent_pid,
            event: DesktopHandshakeEvent::Alive {
                addr: addr.into(),
                token: token.into(),
                transport: DESKTOP_TRANSPORT.to_string(),
                capabilities: vec![
                    DESKTOP_CAPABILITY_HUB_UI.to_string(),
                    DESKTOP_CAPABILITY_WORKSPACE.to_string(),
                ],
            },
        }
    }

    pub fn ready(
        server_version: impl Into<String>,
        pid: u32,
        parent_pid: Option<u32>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            protocol_version: DESKTOP_PROTOCOL_VERSION,
            server_version: server_version.into(),
            pid,
            parent_pid,
            event: DesktopHandshakeEvent::Ready {
                session_id: session_id.into(),
            },
        }
    }

    pub fn session_created(
        server_version: impl Into<String>,
        pid: u32,
        parent_pid: Option<u32>,
        session_id: impl Into<String>,
    ) -> Self {
        Self {
            protocol_version: DESKTOP_PROTOCOL_VERSION,
            server_version: server_version.into(),
            pid,
            parent_pid,
            event: DesktopHandshakeEvent::SessionCreated {
                session_id: session_id.into(),
            },
        }
    }

    pub fn error(
        server_version: impl Into<String>,
        pid: u32,
        parent_pid: Option<u32>,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            protocol_version: DESKTOP_PROTOCOL_VERSION,
            server_version: server_version.into(),
            pid,
            parent_pid,
            event: DesktopHandshakeEvent::Error {
                code: code.into(),
                message: message.into(),
            },
        }
    }

    /// Encode as exactly one newline-terminated protocol record.
    pub fn encode(&self) -> String {
        let json = serde_json::to_string(self).expect("DesktopHandshake must serialize");
        let prefix = match self.event {
            DesktopHandshakeEvent::SessionCreated { .. } => DESKTOP_EVENT_PREFIX,
            _ => DESKTOP_HANDSHAKE_PREFIX,
        };
        format!("{prefix}{json}\n")
    }

    /// Parse a protocol line. Non-protocol stdout returns `Ok(None)`; a line
    /// with our prefix but malformed JSON returns an error so callers can
    /// distinguish pollution from a broken protocol implementation.
    pub fn parse(line: &str) -> Result<Option<Self>, serde_json::Error> {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        let (json, event_prefix) =
            if let Some(json) = trimmed.strip_prefix(DESKTOP_HANDSHAKE_PREFIX) {
                (json, false)
            } else if let Some(json) = trimmed.strip_prefix(DESKTOP_EVENT_PREFIX) {
                (json, true)
            } else {
                return Ok(None);
            };
        let handshake: Self = serde_json::from_str(json)?;
        let session_created = matches!(
            handshake.event,
            DesktopHandshakeEvent::SessionCreated { .. }
        );
        if event_prefix != session_created {
            return Err(serde_json::Error::io(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Desktop handshake phase used the wrong protocol prefix",
            )));
        }
        Ok(Some(handshake))
    }
}

#[cfg(test)]
mod tests;

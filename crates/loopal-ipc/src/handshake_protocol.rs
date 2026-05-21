//! Wire format for the hub-only subprocess handshake.
//!
//! Three lines flow stdout parent ← child:
//!   `LOOPAL_HUB_ALIVE <addr> <token>`   — TCP listener bound, reverse-IPC up
//!   `LOOPAL_HUB_READY <session_id>`     — root agent fully started
//!   `LOOPAL_HUB <addr> <token> <session_id>` — legacy single-line form (kept
//!                                              so older parents still parse)
//! `LOOPAL_HUB_ERROR <single-line msg>` may appear at any point on failure.

const ALIVE_PREFIX: &str = "LOOPAL_HUB_ALIVE ";
const READY_PREFIX: &str = "LOOPAL_HUB_READY ";
const LEGACY_PREFIX: &str = "LOOPAL_HUB ";
const ERROR_PREFIX: &str = "LOOPAL_HUB_ERROR ";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HandshakeLine {
    Alive {
        addr: String,
        token: String,
    },
    Ready {
        session_id: String,
    },
    Legacy {
        addr: String,
        token: String,
        session_id: String,
    },
    Error(String),
}

impl HandshakeLine {
    pub fn encode(&self) -> String {
        match self {
            Self::Alive { addr, token } => format!("{ALIVE_PREFIX}{addr} {token}\n"),
            Self::Ready { session_id } => format!("{READY_PREFIX}{session_id}\n"),
            Self::Legacy {
                addr,
                token,
                session_id,
            } => format!("{LEGACY_PREFIX}{addr} {token} {session_id}\n"),
            Self::Error(msg) => {
                let sanitized: String = msg
                    .chars()
                    .map(|c| if c == '\n' { ' ' } else { c })
                    .collect();
                format!("{ERROR_PREFIX}{sanitized}\n")
            }
        }
    }

    pub fn parse(line: &str) -> Option<Self> {
        let trimmed = line.trim_end_matches(['\r', '\n']);
        if let Some(rest) = trimmed.strip_prefix(ERROR_PREFIX) {
            return Some(Self::Error(rest.to_string()));
        }
        if let Some(rest) = trimmed.strip_prefix(ALIVE_PREFIX) {
            let mut parts = rest.splitn(2, ' ');
            let addr = parts.next()?.to_string();
            let token = parts.next()?.to_string();
            return Some(Self::Alive { addr, token });
        }
        if let Some(rest) = trimmed.strip_prefix(READY_PREFIX) {
            return Some(Self::Ready {
                session_id: rest.to_string(),
            });
        }
        if let Some(rest) = trimmed.strip_prefix(LEGACY_PREFIX) {
            let mut parts = rest.splitn(3, ' ');
            return Some(Self::Legacy {
                addr: parts.next()?.to_string(),
                token: parts.next()?.to_string(),
                session_id: parts.next()?.to_string(),
            });
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_alive() {
        let line = HandshakeLine::Alive {
            addr: "127.0.0.1:42".into(),
            token: "tok".into(),
        };
        assert_eq!(HandshakeLine::parse(&line.encode()), Some(line));
    }

    #[test]
    fn roundtrip_ready() {
        let line = HandshakeLine::Ready {
            session_id: "abc".into(),
        };
        assert_eq!(HandshakeLine::parse(&line.encode()), Some(line));
    }

    #[test]
    fn roundtrip_legacy() {
        let line = HandshakeLine::Legacy {
            addr: "127.0.0.1:1".into(),
            token: "t".into(),
            session_id: "s".into(),
        };
        assert_eq!(HandshakeLine::parse(&line.encode()), Some(line));
    }

    #[test]
    fn error_sanitizes_newlines() {
        let l = HandshakeLine::Error("a\nb".into());
        assert_eq!(l.encode(), "LOOPAL_HUB_ERROR a b\n");
    }

    #[test]
    fn unknown_line_is_none() {
        assert_eq!(HandshakeLine::parse("garbage\n"), None);
    }
}

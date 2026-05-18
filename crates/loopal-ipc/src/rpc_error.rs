use serde_json::Value;

#[derive(Debug, Clone)]
pub enum RpcError {
    Transport(String),
    ChannelDropped,
    Remote {
        code: i64,
        message: String,
        data: Option<Value>,
    },
}

impl RpcError {
    pub fn remote_code(&self) -> Option<i64> {
        match self {
            Self::Remote { code, .. } => Some(*code),
            _ => None,
        }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport(e) => write!(f, "transport: {e}"),
            Self::ChannelDropped => write!(f, "response channel dropped"),
            Self::Remote { code, message, .. } => write!(f, "rpc {code}: {message}"),
        }
    }
}

impl std::error::Error for RpcError {}

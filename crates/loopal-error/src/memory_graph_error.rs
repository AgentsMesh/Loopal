use thiserror::Error;

#[derive(Debug, Error)]
pub enum MemoryGraphError {
    #[error("schema init failed: {0}")]
    SchemaInit(String),

    #[error("watcher: {0}")]
    Watcher(String),

    #[error("invalid node kind: {0}")]
    InvalidNodeKind(String),

    #[error("invalid edge kind: {0}")]
    InvalidEdgeKind(String),

    #[error("invalid provenance: {0}")]
    InvalidProvenance(String),

    #[error("sqlite: {0}")]
    Sqlite(#[from] rusqlite::Error),

    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

#[derive(Debug, Error)]
pub enum MemorySubsystemBootstrapError {
    #[error("memory dir canonicalize failed: {0}")]
    CanonicalizeMemoryDir(std::io::Error),

    #[error("session dir create failed: {0}")]
    CreateSessionDir(std::io::Error),

    #[error("memory graph open failed: {0}")]
    GraphOpen(#[from] MemoryGraphError),
}

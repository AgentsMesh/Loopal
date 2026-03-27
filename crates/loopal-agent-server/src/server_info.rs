//! Server info file for TCP connection discovery.
//!
//! When the agent server starts a TCP listener, it writes connection details
//! to `~/.loopal/run/<pid>.json`. IDE clients read this file to discover the
//! port and authentication token.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use loopal_config::locations::volatile_dir;

/// Connection information written by the agent server.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerInfo {
    pub pid: u32,
    pub port: u16,
    pub token: String,
}

/// Directory for server info files: {temp_dir}/loopal/run/
fn run_dir() -> PathBuf {
    volatile_dir().join("run")
}

/// Path for this process's server info: {run_dir}/<pid>.json
fn info_path() -> PathBuf {
    run_dir().join(format!("{}.json", std::process::id()))
}

/// Write server info to the well-known location (owner-only permissions).
pub fn write_server_info(port: u16, token: &str) -> anyhow::Result<()> {
    let dir = run_dir();
    std::fs::create_dir_all(&dir)?;
    let info = ServerInfo {
        pid: std::process::id(),
        port,
        token: token.to_string(),
    };
    let json = serde_json::to_string_pretty(&info)?;
    let path = info_path();

    // Write with restricted permissions (0600) to protect the token.
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.write_all(json.as_bytes())?;
    }
    #[cfg(not(unix))]
    {
        std::fs::write(&path, &json)?;
    }

    tracing::info!(path = %path.display(), port, "wrote server info");
    Ok(())
}

/// Read server info for a given PID.
pub fn read_server_info(pid: u32) -> anyhow::Result<ServerInfo> {
    let path = run_dir().join(format!("{pid}.json"));
    let content = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&content)?)
}

/// List all available server info files (live servers).
pub fn list_servers() -> Vec<ServerInfo> {
    let dir = run_dir();
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| {
            let content = std::fs::read_to_string(e.path()).ok()?;
            serde_json::from_str::<ServerInfo>(&content).ok()
        })
        .collect()
}

/// Remove this process's server info file (called on shutdown).
pub fn remove_server_info() {
    let path = info_path();
    if path.exists() {
        let _ = std::fs::remove_file(&path);
        tracing::debug!(path = %path.display(), "removed server info");
    }
}

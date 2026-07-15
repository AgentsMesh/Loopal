use std::fs;
use std::path::PathBuf;

use anyhow::{Context as _, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HubDiscoveryRecord {
    pub pid: u32,
    pub tcp_addr: String,
    pub cwd: String,
    pub started_at: String,
    pub root_session_id: String,
}

fn run_dir() -> PathBuf {
    loopal_config::run_dir()
}

fn record_path(pid: u32) -> PathBuf {
    run_dir().join(format!("{pid}.json"))
}

pub fn write_record(record: &HubDiscoveryRecord) -> Result<()> {
    let dir = run_dir();
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let path = record_path(record.pid);
    let json = serde_json::to_string_pretty(record).context("serialize hub record")?;
    fs::write(&path, json).with_context(|| format!("write {}", path.display()))?;
    restrict_to_owner(&path)?;
    Ok(())
}

#[cfg(unix)]
fn restrict_to_owner(path: &std::path::Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let mut perms = fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms).with_context(|| format!("chmod 0600 {}", path.display()))?;
    Ok(())
}

#[cfg(windows)]
fn restrict_to_owner(_path: &std::path::Path) -> Result<()> {
    // NTFS inherits the parent directory ACL; ~/.loopal/run/ defaults
    // to owner+admin only, which matches the Unix 0600 intent.
    Ok(())
}

pub fn remove_record(pid: u32) {
    let _ = fs::remove_file(record_path(pid));
}

pub fn read_record(pid: u32) -> Result<HubDiscoveryRecord> {
    let path = record_path(pid);
    let body =
        fs::read_to_string(&path).with_context(|| format!("read hub record {}", path.display()))?;
    serde_json::from_str(&body).context("parse hub record")
}

pub fn list_records() -> Vec<HubDiscoveryRecord> {
    let dir = run_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return Vec::new();
    };
    let mut records = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("json") {
            continue;
        }
        // Cheap filter: parse PID from filename stem and skip dead
        // entries before paying for read + JSON parse.
        let stem_pid = path
            .file_stem()
            .and_then(|s| s.to_str())
            .and_then(|s| s.parse::<u32>().ok());
        if matches!(stem_pid, Some(p) if !is_alive(p)) {
            continue;
        }
        let body = match fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                debug!(path = %path.display(), error = %e, "list_records: read failed, skipping");
                continue;
            }
        };
        let record: HubDiscoveryRecord = match serde_json::from_str(&body) {
            Ok(r) => r,
            Err(e) => {
                warn!(
                    path = %path.display(), error = %e,
                    "list_records: discovery record is corrupt, skipping"
                );
                continue;
            }
        };
        if is_alive(record.pid) {
            records.push(record);
        }
    }
    records.sort_by(|a, b| b.started_at.cmp(&a.started_at));
    records
}

pub fn cleanup_stale() {
    let dir = run_dir();
    let Ok(entries) = fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        // Only touch files we actually own: <pid>.json and <pid>.sock.
        // Anything else (e.g. a user's `12345.bak`) is left alone.
        let ext = path.extension().and_then(|s| s.to_str());
        if !matches!(ext, Some("json") | Some("sock")) {
            continue;
        }
        let stem = match path.file_stem().and_then(|s| s.to_str()) {
            Some(s) => s,
            None => continue,
        };
        let pid = match stem.parse::<u32>() {
            Ok(p) => p,
            Err(_) => continue,
        };
        if !is_alive(pid) {
            debug!(pid, "discovery: removing stale entry");
            let _ = fs::remove_file(&path);
        }
    }
}

#[cfg(unix)]
pub fn is_alive(pid: u32) -> bool {
    unsafe extern "C" {
        fn kill(pid: i32, sig: i32) -> i32;
    }
    unsafe { kill(pid as i32, 0) == 0 }
}

#[cfg(windows)]
pub fn is_alive(pid: u32) -> bool {
    const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
    type Handle = *mut std::ffi::c_void;
    unsafe extern "system" {
        fn OpenProcess(desired_access: u32, inherit: i32, pid: u32) -> Handle;
        fn CloseHandle(h: Handle) -> i32;
    }
    let h = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if h.is_null() {
        return false;
    }
    unsafe { CloseHandle(h) };
    true
}

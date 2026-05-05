use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context as _, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

fn socket_path(pid: u32) -> PathBuf {
    loopal_config::run_dir().join(format!("{pid}.sock"))
}

pub fn cleanup_channel(pid: u32) {
    let _ = fs::remove_file(socket_path(pid));
}

pub fn bind_token_channel(pid: u32, token: String) -> Result<JoinHandle<()>> {
    let path = socket_path(pid);
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).with_context(|| format!("create {}", dir.display()))?;
    }
    let _ = fs::remove_file(&path);
    let listener = UnixListener::bind(&path)
        .with_context(|| format!("bind unix socket {}", path.display()))?;
    set_owner_only(&path)?;

    let token = std::sync::Arc::new(token);
    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => serve_token(&token, stream).await,
                Err(e) => {
                    debug!(error = %e, "token socket accept ended");
                    return;
                }
            }
        }
    });
    Ok(handle)
}

pub async fn fetch_token(pid: u32) -> Result<String> {
    let path = socket_path(pid);
    let stream = UnixStream::connect(&path)
        .await
        .with_context(|| format!("connect token socket {}", path.display()))?;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();
    tokio::time::timeout(FETCH_TIMEOUT, reader.read_line(&mut line))
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "hub did not produce a token within {}s",
                FETCH_TIMEOUT.as_secs()
            )
        })?
        .context("read token line")?;
    let trimmed = line.trim_end_matches(['\r', '\n']);
    if trimmed.is_empty() {
        anyhow::bail!("hub rejected token request (peer uid mismatch)");
    }
    Ok(trimmed.to_string())
}

async fn serve_token(token: &str, mut stream: UnixStream) {
    if !peer_uid_matches_self(&stream) {
        warn!("token socket: peer uid mismatch, denying request");
        let _ = stream.shutdown().await;
        return;
    }
    let mut payload = String::with_capacity(token.len() + 1);
    payload.push_str(token);
    payload.push('\n');
    if let Err(e) = stream.write_all(payload.as_bytes()).await {
        debug!(error = %e, "token socket: write failed");
    }
    let _ = stream.shutdown().await;
}

fn peer_uid_matches_self(stream: &UnixStream) -> bool {
    use std::os::fd::AsRawFd;
    let fd = stream.as_raw_fd();
    let Some(peer_uid) = peer_uid_ffi(fd) else {
        warn!("peer_uid lookup failed");
        return false;
    };
    peer_uid == self_uid()
}

#[cfg(target_os = "linux")]
fn peer_uid_ffi(fd: i32) -> Option<u32> {
    #[repr(C)]
    struct UCred {
        pid: i32,
        uid: u32,
        gid: u32,
    }
    const SOL_SOCKET: i32 = 1;
    const SO_PEERCRED: i32 = 17;
    unsafe extern "C" {
        fn getsockopt(
            s: i32,
            level: i32,
            name: i32,
            val: *mut std::ffi::c_void,
            len: *mut u32,
        ) -> i32;
    }
    let mut cred = UCred {
        pid: 0,
        uid: 0,
        gid: 0,
    };
    let mut len = std::mem::size_of::<UCred>() as u32;
    let ret = unsafe {
        getsockopt(
            fd,
            SOL_SOCKET,
            SO_PEERCRED,
            &mut cred as *mut _ as *mut std::ffi::c_void,
            &mut len,
        )
    };
    if ret == 0 { Some(cred.uid) } else { None }
}

#[cfg(any(
    target_os = "macos",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
fn peer_uid_ffi(fd: i32) -> Option<u32> {
    unsafe extern "C" {
        fn getpeereid(s: i32, euid: *mut u32, egid: *mut u32) -> i32;
    }
    let mut uid: u32 = 0;
    let mut gid: u32 = 0;
    let ret = unsafe { getpeereid(fd, &mut uid, &mut gid) };
    if ret == 0 { Some(uid) } else { None }
}

fn self_uid() -> u32 {
    unsafe extern "C" {
        fn getuid() -> u32;
    }
    unsafe { getuid() }
}

fn set_owner_only(path: &Path) -> Result<()> {
    let mut perms = fs::metadata(path)
        .with_context(|| format!("stat {}", path.display()))?
        .permissions();
    perms.set_mode(0o600);
    fs::set_permissions(path, perms).with_context(|| format!("chmod 0600 {}", path.display()))?;
    Ok(())
}

use std::os::windows::io::AsRawHandle as _;
use std::time::Duration;

use anyhow::{Context as _, Result};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeServer, ServerOptions};
use tokio::task::JoinHandle;
use tracing::{debug, warn};

use super::windows_sid::query_user_sid;

const FETCH_TIMEOUT: Duration = Duration::from_secs(5);

unsafe extern "system" {
    fn GetNamedPipeClientProcessId(pipe: *mut std::ffi::c_void, pid: *mut u32) -> i32;
}

fn client_process_id(server: &NamedPipeServer) -> Option<u32> {
    let handle = server.as_raw_handle() as *mut std::ffi::c_void;
    let mut pid: u32 = 0;
    let ok = unsafe { GetNamedPipeClientProcessId(handle, &mut pid) };
    if ok != 0 { Some(pid) } else { None }
}

fn pipe_name(pid: u32) -> String {
    format!(r"\\.\pipe\loopal-hub-{pid}")
}

pub fn cleanup_channel(_pid: u32) {
    // Named Pipes have no on-disk presence; the OS releases the kernel
    // object when the last server handle closes (i.e. process exit).
}

pub fn bind_token_channel(pid: u32, token: String) -> Result<JoinHandle<()>> {
    let name = pipe_name(pid);
    let mut server = ServerOptions::new()
        .first_pipe_instance(true)
        .create(&name)
        .with_context(|| format!("create named pipe {name}"))?;

    let token = std::sync::Arc::new(token);
    let name_loop = name.clone();
    let handle = tokio::spawn(async move {
        loop {
            if server.connect().await.is_err() {
                debug!("named pipe connect ended");
                return;
            }
            let connected = server;
            let next = match ServerOptions::new().create(&name_loop) {
                Ok(s) => s,
                Err(e) => {
                    warn!(error = %e, "named pipe re-create failed");
                    serve_token(&token, connected).await;
                    return;
                }
            };
            server = next;
            let token_for_serve = token.clone();
            tokio::spawn(async move {
                serve_token(&token_for_serve, connected).await;
            });
        }
    });
    Ok(handle)
}

pub async fn fetch_token(pid: u32) -> Result<String> {
    let name = pipe_name(pid);
    let client = ClientOptions::new()
        .open(&name)
        .with_context(|| format!("open named pipe {name}"))?;
    let mut reader = BufReader::new(client);
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
        anyhow::bail!("hub rejected token request (peer SID mismatch)");
    }
    Ok(trimmed.to_string())
}

async fn serve_token(token: &str, mut server: NamedPipeServer) {
    let client_pid = match client_process_id(&server) {
        Some(p) => p,
        None => {
            warn!("GetNamedPipeClientProcessId failed; denying");
            let _ = server.shutdown().await;
            return;
        }
    };
    if !peer_owned_by_self(client_pid) {
        warn!(client_pid, "named pipe peer SID mismatch, denying");
        let _ = server.shutdown().await;
        return;
    }
    let mut payload = String::with_capacity(token.len() + 1);
    payload.push_str(token);
    payload.push('\n');
    if let Err(e) = server.write_all(payload.as_bytes()).await {
        debug!(error = %e, "named pipe write failed");
    }
    let _ = server.shutdown().await;
}

fn peer_owned_by_self(pid: u32) -> bool {
    let peer = match query_user_sid(Some(pid)) {
        Some(sid) => sid,
        None => return false,
    };
    let me = match query_user_sid(None) {
        Some(sid) => sid,
        None => return false,
    };
    peer == me
}

use std::path::Path;

pub fn is_agent_available() -> bool {
    match std::env::var("SSH_AUTH_SOCK") {
        Ok(sock) => !sock.is_empty() && Path::new(&sock).exists(),
        Err(_) => false,
    }
}

pub fn passphrase_warning(is_encrypted: bool) -> Option<&'static str> {
    if is_encrypted && !is_agent_available() {
        Some(
            "SSH key is passphrase-protected and no ssh-agent socket is available. \
             Vault operations will be rejected. Run `ssh-add <key>` first to load the \
             key into the running agent.",
        )
    } else {
        None
    }
}

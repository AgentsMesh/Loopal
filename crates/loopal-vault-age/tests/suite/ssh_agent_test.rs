use loopal_vault_age::{is_agent_available, passphrase_warning};
use tempfile::tempdir;

fn with_env<F: FnOnce()>(key: &str, value: Option<&str>, f: F) {
    let orig = std::env::var(key).ok();
    unsafe {
        match value {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
    f();
    unsafe {
        match orig {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}

#[test]
fn agent_unavailable_when_env_unset() {
    with_env("SSH_AUTH_SOCK", None, || {
        assert!(!is_agent_available());
    });
}

#[test]
fn agent_unavailable_when_socket_path_missing() {
    with_env("SSH_AUTH_SOCK", Some("/nonexistent/socket/path"), || {
        assert!(!is_agent_available());
    });
}

#[test]
fn agent_unavailable_when_env_empty() {
    with_env("SSH_AUTH_SOCK", Some(""), || {
        assert!(!is_agent_available());
    });
}

#[test]
fn agent_available_when_socket_path_exists() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("ssh-agent.sock");
    std::fs::write(&sock, "").unwrap();
    with_env("SSH_AUTH_SOCK", Some(sock.to_str().unwrap()), || {
        assert!(is_agent_available());
    });
}

#[test]
fn passphrase_warning_silent_for_unencrypted() {
    with_env("SSH_AUTH_SOCK", None, || {
        assert!(passphrase_warning(false).is_none());
    });
}

#[test]
fn passphrase_warning_when_encrypted_without_agent() {
    with_env("SSH_AUTH_SOCK", None, || {
        let w = passphrase_warning(true);
        assert!(w.is_some());
        assert!(w.unwrap().contains("ssh-add"));
    });
}

#[test]
fn passphrase_warning_silent_when_encrypted_with_agent() {
    let dir = tempdir().unwrap();
    let sock = dir.path().join("agent.sock");
    std::fs::write(&sock, "").unwrap();
    with_env("SSH_AUTH_SOCK", Some(sock.to_str().unwrap()), || {
        assert!(passphrase_warning(true).is_none());
    });
}

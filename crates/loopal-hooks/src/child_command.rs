use std::process::Stdio;

use loopal_protocol::META_HUB_TOKEN_ENV;
use tokio::process::Command;

pub(crate) fn shell_command(value: &str) -> Command {
    let mut command = Command::new("sh");
    command
        .arg("-c")
        .arg(value)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .env_remove(META_HUB_TOKEN_ENV);
    command
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use loopal_protocol::META_HUB_TOKEN_ENV;

    use super::shell_command;

    #[test]
    fn hook_command_removes_meta_token() {
        let command = shell_command("true");
        let removed = command
            .as_std()
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(META_HUB_TOKEN_ENV));
        assert_eq!(removed.and_then(|(_, value)| value), None);
        assert!(removed.is_some());
    }
}

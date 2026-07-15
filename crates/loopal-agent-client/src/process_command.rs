use std::path::Path;
use std::process::Stdio;

use loopal_protocol::META_HUB_TOKEN_ENV;
use tokio::process::Command;

pub(crate) fn agent_command(executable: &Path, env_vars: &[(&str, &str)]) -> Command {
    let mut command = Command::new(executable);
    command
        .arg("--serve")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (key, value) in env_vars {
        command.env(key, value);
    }
    command.env_remove(META_HUB_TOKEN_ENV);
    command
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::OsString;
    use std::path::Path;

    use loopal_protocol::META_HUB_TOKEN_ENV;

    use super::agent_command;

    #[test]
    fn agent_command_removes_inherited_and_explicit_meta_token() {
        let command = agent_command(
            Path::new("loopal"),
            &[(META_HUB_TOKEN_ENV, "secret"), ("LOOPAL_KEEP", "visible")],
        );
        let env = command
            .as_std()
            .get_envs()
            .map(|(key, value)| (key.to_owned(), value.map(OsString::from)))
            .collect::<HashMap<_, _>>();
        assert_eq!(
            env.get(std::ffi::OsStr::new(META_HUB_TOKEN_ENV)),
            Some(&None)
        );
        assert_eq!(
            env.get(std::ffi::OsStr::new("LOOPAL_KEEP"))
                .and_then(|value| value.as_deref()),
            Some(std::ffi::OsStr::new("visible"))
        );
    }
}

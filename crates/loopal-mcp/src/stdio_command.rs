use std::collections::HashMap;

use loopal_protocol::META_HUB_TOKEN_ENV;
use tokio::process::Command;

pub(crate) fn stdio_command(
    executable: &str,
    args: &[String],
    env: &HashMap<String, String>,
) -> Command {
    let mut command = Command::new(executable);
    command.args(args);
    for (key, value) in env {
        command.env(key, value);
    }
    command.env_remove(META_HUB_TOKEN_ENV);
    command.kill_on_drop(true);
    command
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::ffi::{OsStr, OsString};

    use loopal_protocol::META_HUB_TOKEN_ENV;

    use super::stdio_command;

    #[test]
    fn stdio_command_removes_meta_token_after_provider_env() {
        let env = HashMap::from([
            (META_HUB_TOKEN_ENV.to_string(), "secret".to_string()),
            ("MCP_KEEP".to_string(), "visible".to_string()),
        ]);
        let command = stdio_command("server", &[], &env);
        let overrides = command
            .as_std()
            .get_envs()
            .map(|(key, value)| (key.to_owned(), value.map(OsString::from)))
            .collect::<HashMap<_, _>>();
        assert_eq!(overrides.get(OsStr::new(META_HUB_TOKEN_ENV)), Some(&None));
        assert_eq!(
            overrides
                .get(OsStr::new("MCP_KEEP"))
                .and_then(|value| value.as_deref()),
            Some(OsStr::new("visible"))
        );
    }
}

#![cfg(not(windows))]

use std::time::Duration;

use loopal_backend::shell::exec_command;
use loopal_protocol::META_HUB_TOKEN_ENV;
use loopal_tool_api::backend_types::EnvOverride;

#[tokio::test]
async fn bash_process_cannot_restore_explicit_meta_hub_token() {
    let environment = EnvOverride::new().with(META_HUB_TOKEN_ENV, "root-agent-secret");
    let result = exec_command(
        &std::env::temp_dir(),
        None,
        r#"printf '%s' "${LOOPAL_META_HUB_TOKEN-unset}""#,
        &environment,
        Duration::from_secs(5),
        "secret-env-test",
    )
    .await
    .unwrap();
    assert_eq!(result.exit_code, 0);
    assert_eq!(result.stdout.trim(), "unset");
}

use serde_json::json;

use crate::bash_secret_support::{CANARY, run, test_context};

#[cfg(unix)]
#[tokio::test]
async fn foreground_bash_redacts_before_result_and_log_persistence() {
    let session_id = format!("fg-secret-{}", std::process::id());
    let (kernel, ctx) = test_context(&session_id);

    let output = run(
        &kernel,
        &ctx,
        "foreground-secret",
        "Bash",
        json!({
            "command": "printf '%s\\n' \"$TOKEN\"",
            "env": {"TOKEN": "<secret_ref:token>"}
        }),
    )
    .await;
    assert!(!output.content.contains(CANARY));
    assert_eq!(output.content.trim(), "<secret_ref:token>");

    let dir = loopal_backend::session_bash_dir(&session_id);
    let log_path = std::fs::read_dir(dir)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.extension().is_some_and(|ext| ext == "log"))
        .unwrap();
    let log = std::fs::read_to_string(log_path).unwrap();
    assert!(!log.contains(CANARY));
    assert_eq!(log, "<secret_ref:token>\n");
}

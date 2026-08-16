use std::time::Duration;

use crate::cli::{ChildPassthroughArgs, Cli, ParentOnlyArgs};

use super::run;

fn config(root: &std::path::Path) -> loopal_config::ResolvedConfig {
    let settings = loopal_config::Settings {
        model: "claude-opus-4-8".into(),
        telemetry: loopal_config::TelemetryConfig {
            telemetry_dir: Some(root.join("telemetry").display().to_string()),
            ..Default::default()
        },
        ..Default::default()
    };
    loopal_config::ResolvedConfig {
        settings,
        workflow_preset_thinking_recommendation: None,
        mcp_servers: Default::default(),
        skills: Default::default(),
        hooks: Vec::new(),
        instructions: String::new(),
        memory: String::new(),
        classifier_prompt: None,
        layers: Vec::new(),
        secrets: None,
    }
}

#[tokio::test]
#[ignore = "real-process Bazel coverage producer"]
async fn ephemeral_server_runs_real_agent_and_shuts_down() {
    for variable in ["LOOPAL_BINARY", "LOOPAL_TEST_PROVIDER"] {
        let path = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} must be set"));
        assert!(std::path::Path::new(&path).is_file(), "missing {variable}");
    }

    let project = tempfile::tempdir().expect("create isolated server project");
    let cli = Cli {
        child: ChildPassthroughArgs {
            permission: Some("yolo".into()),
            ephemeral: true,
            ..Default::default()
        },
        parent_only: ParentOnlyArgs {
            server: true,
            ..Default::default()
        },
        prompt: vec!["server".into(), "lifecycle".into()],
    };

    tokio::time::timeout(
        Duration::from_secs(30),
        run(&cli, project.path(), &config(project.path())),
    )
    .await
    .expect("server lifecycle exceeded test deadline")
    .expect("server lifecycle failed");
}

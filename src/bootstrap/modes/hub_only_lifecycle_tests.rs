use std::process::Stdio;
use std::time::Duration;

use loopal_ipc::protocol::methods;
use tokio::sync::oneshot;

use crate::bootstrap::lifecycle_test_support::{
    EnvGuard, assert_runtime_fixture, cli, config, register_ui, wait_for_record,
};
use crate::cli::ParentOnlyArgs;

use super::{StartupProtocol, run, run_desktop, run_with_protocol_observed};

#[tokio::test]
#[ignore = "real-process Bazel coverage producer"]
async fn hub_only_runs_enabled_workflow_runtime_until_remote_shutdown() {
    let _fixtures = assert_runtime_fixture();
    let home = tempfile::tempdir().expect("create Hub-only coverage home");
    let _home = EnvGuard::set("HOME", home.path());
    let project = tempfile::tempdir().expect("create Hub-only coverage project");
    let cli = cli(ParentOnlyArgs {
        hub_only: true,
        ..Default::default()
    });
    let config = config(home.path(), true);
    let task = tokio::spawn(async move { run(&cli, project.path(), &config, None).await });

    let pid = std::process::id();
    let record = wait_for_record(pid).await;
    assert!(!record.root_session_id.is_empty());
    super::super::hub_cli::run_kill_hub(pid)
        .await
        .expect("send Hub-only shutdown");
    tokio::time::timeout(Duration::from_secs(30), task)
        .await
        .expect("Hub-only shutdown deadline")
        .expect("join Hub-only lifecycle")
        .expect("Hub-only lifecycle");
    assert!(super::super::discovery::read_record(pid).is_err());
}

#[tokio::test]
#[ignore = "real-process Bazel coverage producer"]
async fn missing_resume_reports_startup_failure_and_rolls_back() {
    let _fixtures = assert_runtime_fixture();
    let home = tempfile::tempdir().expect("create resume coverage home");
    let _home = EnvGuard::set("HOME", home.path());
    let project = tempfile::tempdir().expect("create resume coverage project");
    let cli = cli(ParentOnlyArgs {
        hub_only: true,
        ..Default::default()
    });

    let error = tokio::time::timeout(
        Duration::from_secs(30),
        run(
            &cli,
            project.path(),
            &config(home.path(), false),
            Some("missing-bootstrap-session"),
        ),
    )
    .await
    .expect("missing resume deadline")
    .expect_err("missing resume must fail");
    assert!(!error.to_string().is_empty());
}

#[tokio::test]
#[ignore = "real-process Bazel coverage producer"]
#[cfg(unix)]
async fn desktop_entrypoint_rejects_a_parent_that_exited_during_startup() {
    let _fixtures = assert_runtime_fixture();
    let home = tempfile::tempdir().expect("create Desktop failure coverage home");
    let _home = EnvGuard::set("HOME", home.path());
    let project = tempfile::tempdir().expect("create Desktop failure coverage project");
    let mut parent = std::process::Command::new("sh")
        .args(["-c", "exit 0"])
        .spawn()
        .expect("spawn exiting Desktop parent");
    let parent_pid = parent.id();
    parent.wait().expect("reap exiting Desktop parent");
    let cli = cli(ParentOnlyArgs {
        hub_only: true,
        ..Default::default()
    });

    let error = tokio::time::timeout(
        Duration::from_secs(15),
        run_desktop(
            &cli,
            project.path(),
            &config(home.path(), false),
            None,
            Some(parent_pid),
        ),
    )
    .await
    .expect("Desktop parent-exit deadline")
    .expect_err("an exited Desktop parent must abort startup");
    assert!(error.to_string().contains("exited during startup"));
}

#[tokio::test]
#[ignore = "real-process Bazel coverage producer"]
#[cfg(unix)]
async fn desktop_covers_parent_exit_and_hub_shutdown_paths() {
    let _fixtures = assert_runtime_fixture();
    let home = tempfile::tempdir().expect("create Desktop coverage home");
    let _home = EnvGuard::set("HOME", home.path());
    for parent_exits in [true, false] {
        run_desktop_case(home.path(), parent_exits).await;
    }
}

#[cfg(unix)]
async fn run_desktop_case(home: &std::path::Path, parent_exits: bool) {
    let project = tempfile::tempdir().expect("create Desktop coverage project");
    let mut parent = std::process::Command::new("sh")
        .args(["-c", "read line"])
        .stdin(Stdio::piped())
        .spawn()
        .expect("spawn blocking Desktop parent");
    let parent_pid = parent.id();
    let cli = cli(ParentOnlyArgs {
        hub_only: true,
        require_ui_ready: true,
        ..Default::default()
    });
    let config = config(home, false);
    let (alive_tx, alive_rx) = oneshot::channel();
    let task = tokio::spawn(async move {
        run_with_protocol_observed(
            &cli,
            project.path(),
            &config,
            None,
            StartupProtocol::Desktop {
                parent_pid: Some(parent_pid),
            },
            Some(alive_tx),
        )
        .await
    });
    let (addr, token) = tokio::time::timeout(Duration::from_secs(15), alive_rx)
        .await
        .expect("Desktop ALIVE deadline")
        .expect("Desktop ALIVE channel");
    let (connection, _incoming) = register_ui(&addr, &token).await;
    wait_for_record(std::process::id()).await;

    let result = if parent_exits {
        parent.kill().expect("terminate Desktop parent");
        parent.wait().expect("reap Desktop parent");
        tokio::time::timeout(Duration::from_secs(30), task).await
    } else {
        connection
            .send_request(methods::HUB_SHUTDOWN.name, serde_json::json!({}))
            .await
            .expect("send Desktop Hub shutdown");
        let result = tokio::time::timeout(Duration::from_secs(30), task).await;
        parent.kill().expect("terminate surviving Desktop parent");
        parent.wait().expect("reap surviving Desktop parent");
        result
    };
    result
        .expect("Desktop shutdown deadline")
        .expect("join Desktop lifecycle")
        .expect("Desktop lifecycle");
}

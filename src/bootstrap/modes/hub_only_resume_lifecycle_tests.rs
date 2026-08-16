use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::bootstrap::lifecycle_test_support::{
    EnvGuard, assert_runtime_fixture, cli, config, wait_for_record,
};
use crate::cli::ParentOnlyArgs;

use super::run;

#[tokio::test]
#[ignore = "real-process Bazel coverage producer"]
async fn hub_only_can_resume_the_session_created_by_an_earlier_run() {
    let _fixtures = assert_runtime_fixture();
    let home = tempfile::tempdir().expect("create Hub-only resume home");
    let _home = EnvGuard::set("HOME", home.path());
    let project = tempfile::tempdir().expect("create Hub-only resume project");

    let session_id = run_and_stop(home.path(), project.path(), None).await;
    let resumed = run_and_stop(home.path(), project.path(), Some(session_id.clone())).await;

    assert_eq!(resumed, session_id);
}

async fn run_and_stop(home: &Path, project: &Path, resume: Option<String>) -> String {
    let expected_session = resume.clone();
    let project = PathBuf::from(project);
    let cli = cli(ParentOnlyArgs {
        hub_only: true,
        ..Default::default()
    });
    let config = config(home, false);
    let task = tokio::spawn(async move { run(&cli, &project, &config, resume.as_deref()).await });

    let pid = std::process::id();
    let record = wait_for_record(pid).await;
    if let Some(expected) = expected_session {
        assert_eq!(record.root_session_id, expected);
    }
    super::super::hub_cli::run_kill_hub(pid)
        .await
        .expect("send Hub-only shutdown");
    tokio::time::timeout(Duration::from_secs(30), task)
        .await
        .expect("Hub-only shutdown deadline")
        .expect("join Hub-only lifecycle")
        .expect("Hub-only lifecycle");
    assert!(super::super::discovery::read_record(pid).is_err());

    record.root_session_id
}

use std::time::Duration;

use crate::cli::ParentOnlyArgs;

use super::run;
use crate::bootstrap::lifecycle_test_support::{EnvGuard, assert_runtime_fixture, cli, config};

#[tokio::test]
#[ignore = "real-process Bazel coverage producer"]
async fn acp_entrypoint_owns_a_real_hub_and_agent_lifecycle() {
    assert_runtime_fixture();
    let home = tempfile::tempdir().expect("create ACP coverage home");
    let _home = EnvGuard::set("HOME", home.path());
    let project = tempfile::tempdir().expect("create ACP coverage project");
    let cli = cli(ParentOnlyArgs {
        acp: true,
        ..Default::default()
    });

    tokio::time::timeout(
        Duration::from_secs(30),
        run(&cli, project.path(), &config(home.path(), true)),
    )
    .await
    .expect("ACP lifecycle deadline")
    .expect("ACP lifecycle");
}

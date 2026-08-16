use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use super::*;
use support::{env_lock, missing_path, script, unique_id};

#[path = "tests/support.rs"]
mod support;

#[test]
fn executable_resolver_obeys_precedence() {
    let override_path = script("exit 0");
    let explicit = script("exit 0");
    let current = script("exit 0");
    assert_eq!(
        AgentProcess::select_executable(
            explicit.to_str().unwrap(),
            Some(override_path.clone()),
            Some(current.clone()),
        ),
        override_path
    );
    assert_eq!(
        AgentProcess::select_executable(explicit.to_str().unwrap(), None, Some(current.clone())),
        explicit
    );
    assert_eq!(
        AgentProcess::select_executable("loopal", Some(missing_path()), Some(current.clone())),
        current
    );
    assert_eq!(
        AgentProcess::select_executable("loopal", Some(missing_path()), None),
        Path::new("loopal")
    );
}

#[tokio::test]
async fn spawn_now_some_succeeds_without_test_binary() {
    let _lock = env_lock().await;
    let old = std::env::var_os("LOOPAL_BINARY");
    unsafe { std::env::remove_var("LOOPAL_BINARY") };
    let path = script("exit 0");
    let process = AgentProcess::spawn_now(Some(path.to_str().unwrap())).unwrap();
    assert!(process.pid().is_some());
    assert!(process.transport().is_connected());
    let status = process.wait().await.unwrap();
    assert!(status.success());
    match old {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
}

#[tokio::test]
async fn spawn_now_none_uses_override_and_shutdowns() {
    let _lock = env_lock().await;
    let old = std::env::var_os("LOOPAL_BINARY");
    let path = script("while read line; do :; done");
    unsafe { std::env::set_var("LOOPAL_BINARY", &path) };
    let process = AgentProcess::spawn_now(None).unwrap();
    assert!(process.pid().is_some());
    process.shutdown().await.unwrap();
    match old {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
}

#[tokio::test]
async fn relative_override_executes_the_existing_file_not_path_lookup() {
    let _lock = env_lock().await;
    let old_override = std::env::var_os("LOOPAL_BINARY");
    let old_cwd = std::env::current_dir().unwrap();
    let root = std::env::temp_dir().join(format!(
        "loopal-relative-process-{}-{}",
        std::process::id(),
        unique_id()
    ));
    fs::create_dir_all(&root).unwrap();
    let executable = root.join("loopal-relative-fixture");
    fs::write(&executable, "#!/bin/sh\nwhile read line; do :; done\n").unwrap();
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_current_dir(&root).unwrap();
    unsafe { std::env::set_var("LOOPAL_BINARY", "loopal-relative-fixture") };

    let process = AgentProcess::spawn_now(None).unwrap();
    assert!(process.pid().is_some());
    process.shutdown().await.unwrap();

    std::env::set_current_dir(old_cwd).unwrap();
    match old_override {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
    fs::remove_dir_all(root).unwrap();
}

#[tokio::test]
async fn spawn_with_env_forwards_environment_and_waits() {
    let _lock = env_lock().await;
    let old = std::env::var_os("LOOPAL_BINARY");
    unsafe { std::env::remove_var("LOOPAL_BINARY") };
    let path = script("test \"$LOOPAL_FIXTURE\" = expected");
    let process = AgentProcess::spawn_with_env_now(
        Some(path.to_str().unwrap()),
        &[("LOOPAL_FIXTURE", "expected")],
    )
    .unwrap();
    assert!(process.wait().await.unwrap().success());
    match old {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
}

#[tokio::test]
async fn async_spawn_wrappers_and_running_state_work() {
    let _lock = env_lock().await;
    let old = std::env::var_os("LOOPAL_BINARY");
    unsafe { std::env::remove_var("LOOPAL_BINARY") };
    let path = script("exec sleep 1");
    let mut process = AgentProcess::spawn(Some(path.to_str().unwrap()))
        .await
        .unwrap();
    assert!(process.is_running());
    process.wait_or_kill(Duration::from_millis(10)).await;

    let path = script("exit 0");
    let process =
        AgentProcess::spawn_with_env(Some(path.to_str().unwrap()), &[("LOOPAL_UNUSED", "value")])
            .await
            .unwrap();
    process.wait_or_kill(Duration::from_secs(1)).await;
    match old {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
}

#[tokio::test(flavor = "current_thread")]
async fn current_thread_runtime_spawn_is_safe() {
    let _lock = env_lock().await;
    let old = std::env::var_os("LOOPAL_BINARY");
    let path = script("while read line; do :; done");
    unsafe { std::env::set_var("LOOPAL_BINARY", &path) };
    AgentProcess::spawn(None)
        .await
        .unwrap()
        .shutdown()
        .await
        .unwrap();
    match old {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
}

#[tokio::test]
async fn shutdown_kills_child_that_ignores_eof() {
    let _lock = env_lock().await;
    let old = std::env::var_os("LOOPAL_BINARY");
    unsafe { std::env::remove_var("LOOPAL_BINARY") };
    let path = script("trap '' TERM; exec sleep 30");
    AgentProcess::spawn_now(Some(path.to_str().unwrap()))
        .unwrap()
        .shutdown()
        .await
        .unwrap();
    match old {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
}

#[tokio::test]
async fn spawn_failure_and_wait_or_kill_are_bounded() {
    let _lock = env_lock().await;
    let old = std::env::var_os("LOOPAL_BINARY");
    unsafe { std::env::remove_var("LOOPAL_BINARY") };
    let invalid = script("exit 0");
    fs::set_permissions(&invalid, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(AgentProcess::spawn_now(Some(invalid.to_str().unwrap())).is_err());
    let path = script("exec sleep 30");
    AgentProcess::spawn_now(Some(path.to_str().unwrap()))
        .unwrap()
        .wait_or_kill(Duration::from_millis(10))
        .await;
    match old {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
}

#[path = "tests/branch_close.rs"]
mod branch_close;
#[path = "wait_error_tests.rs"]
mod wait_error_tests;

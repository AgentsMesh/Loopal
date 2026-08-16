use std::ffi::c_int;
use std::time::Duration;

use super::super::AgentProcess;
use super::support::{env_lock, script};

unsafe extern "C" {
    fn waitpid(pid: c_int, status: *mut c_int, options: c_int) -> c_int;
}

#[tokio::test]
async fn wait_or_kill_handles_a_child_reaped_outside_tokio() {
    let _lock = env_lock().await;
    let old_override = std::env::var_os("LOOPAL_BINARY");
    unsafe { std::env::remove_var("LOOPAL_BINARY") };
    let path = script("exit 0");
    let process = AgentProcess::spawn_now(Some(path.to_str().unwrap())).unwrap();
    let pid = process.pid().expect("spawned child pid") as c_int;
    let mut status = 0;

    // SAFETY: `pid` belongs to the live child above and `status` is writable.
    let reaped = unsafe { waitpid(pid, &mut status, 0) };
    assert_eq!(reaped, pid);
    process.wait_or_kill(Duration::from_secs(1)).await;

    match old_override {
        Some(value) => unsafe { std::env::set_var("LOOPAL_BINARY", value) },
        None => unsafe { std::env::remove_var("LOOPAL_BINARY") },
    }
}

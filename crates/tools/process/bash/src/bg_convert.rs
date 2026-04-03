//! Convert a spawned or timed-out process into a background task.
//!
//! Two entry points:
//! - [`register`] — for `ExecOutcome::TimedOut` (has child + accumulated buffers)
//! - [`register_spawned`] — for `Backend::exec_background` (fresh child, no buffers yet)

use std::sync::{Arc, Mutex};

use loopal_backend::shell::SpawnedBackgroundData;
use loopal_backend::shell_stream::TimedOutProcessData;
use loopal_error::ProcessHandle;
use loopal_tool_background::{BackgroundTaskStore, TaskStatus};

use crate::bg_monitor::{
    build_task, combine, insert_task, read_pipe, spawn_monitor_with_cleanup, truncate_cmd,
};

// ── Timed-out process (streaming → background) ──────────────────────

pub fn register(
    store: &BackgroundTaskStore,
    handle: ProcessHandle,
    command: &str,
) -> Option<String> {
    let data = handle.0.downcast::<TimedOutProcessData>().ok()?;
    let desc = format!("(auto-bg) {}", truncate_cmd(command, 60));
    Some(register_timed_out(store, *data, &desc))
}

fn register_timed_out(
    store: &BackgroundTaskStore,
    data: TimedOutProcessData,
    desc: &str,
) -> String {
    let TimedOutProcessData {
        child,
        stdout_buf,
        stderr_buf,
        abort_handles,
    } = data;

    let task_id = store.generate_task_id();
    let combined_output = Arc::new(Mutex::new(combine(&stdout_buf, &stderr_buf)));
    let exit_code = Arc::new(Mutex::new(None));
    let status = Arc::new(Mutex::new(TaskStatus::Running));
    let (watch_tx, watch_rx) = tokio::sync::watch::channel(TaskStatus::Running);

    insert_task(
        store,
        &task_id,
        build_task(
            &combined_output,
            &exit_code,
            &status,
            &child,
            desc,
            watch_rx,
        ),
    );

    let ob = Arc::clone(&stdout_buf);
    let eb = Arc::clone(&stderr_buf);
    spawn_monitor_with_cleanup(
        child,
        combined_output,
        exit_code,
        status,
        watch_tx,
        abort_handles,
        move || combine(&ob, &eb),
    );

    task_id
}

// ── Freshly spawned process (run_in_background → store) ─────────────

pub fn register_spawned(
    store: &BackgroundTaskStore,
    handle: ProcessHandle,
    desc: &str,
) -> Option<String> {
    let data = handle.0.downcast::<SpawnedBackgroundData>().ok()?;
    Some(register_spawned_data(store, *data, desc))
}

fn register_spawned_data(
    store: &BackgroundTaskStore,
    data: SpawnedBackgroundData,
    desc: &str,
) -> String {
    let stdout_pipe;
    let stderr_pipe;
    {
        let mut guard = data.child.lock().unwrap();
        let child = guard.as_mut().expect("child already taken");
        stdout_pipe = child.stdout.take();
        stderr_pipe = child.stderr.take();
    }

    let stdout_buf = Arc::new(Mutex::new(String::new()));
    let stderr_buf = Arc::new(Mutex::new(String::new()));
    let combined_output = Arc::new(Mutex::new(String::new()));
    let exit_code = Arc::new(Mutex::new(None));
    let status = Arc::new(Mutex::new(TaskStatus::Running));
    let (watch_tx, watch_rx) = tokio::sync::watch::channel(TaskStatus::Running);
    let task_id = store.generate_task_id();

    insert_task(
        store,
        &task_id,
        build_task(
            &combined_output,
            &exit_code,
            &status,
            &data.child,
            desc,
            watch_rx,
        ),
    );

    let mut reader_aborts = Vec::new();
    if let Some(pipe) = stdout_pipe {
        let buf = Arc::clone(&stdout_buf);
        let h = tokio::spawn(async move { read_pipe(&buf, pipe).await });
        reader_aborts.push(h.abort_handle());
    }
    if let Some(pipe) = stderr_pipe {
        let buf = Arc::clone(&stderr_buf);
        let h = tokio::spawn(async move { read_pipe(&buf, pipe).await });
        reader_aborts.push(h.abort_handle());
    }

    let ob = Arc::clone(&stdout_buf);
    let eb = Arc::clone(&stderr_buf);
    spawn_monitor_with_cleanup(
        data.child,
        combined_output,
        exit_code,
        status,
        watch_tx,
        reader_aborts,
        move || combine(&ob, &eb),
    );

    task_id
}

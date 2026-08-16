use std::path::PathBuf;
use std::sync::Arc;

use loopal_backend::shell::SpawnedBackgroundData;
use loopal_backend::shell_stream::TimedOutProcessData;
use loopal_error::ProcessHandle;
use loopal_tool_background::BackgroundTaskStore;

pub(crate) fn register(
    store: &Arc<BackgroundTaskStore>,
    handle: ProcessHandle,
    command: &str,
) -> Result<(String, PathBuf), &'static str> {
    let data = handle
        .0
        .downcast::<TimedOutProcessData>()
        .map_err(|_| "timed-out process adoption failed")?;
    let TimedOutProcessData {
        spawned,
        log_path,
        capture_state,
        capture_task,
    } = *data;
    let description = format!("(auto-bg) {}", truncate_cmd_for_desc(command, 60));
    let id = store.spawn_process_task(
        spawned,
        log_path.clone(),
        capture_state,
        capture_task,
        &description,
    );
    Ok((id, log_path))
}

pub(crate) fn register_spawned(
    store: &Arc<BackgroundTaskStore>,
    handle: ProcessHandle,
    description: &str,
) -> Result<(String, PathBuf), &'static str> {
    let data = handle
        .0
        .downcast::<SpawnedBackgroundData>()
        .map_err(|_| "background process adoption failed")?;
    let SpawnedBackgroundData {
        spawned,
        log_path,
        capture_state,
        capture_task,
    } = *data;
    let id = store.spawn_process_task(
        spawned,
        log_path.clone(),
        capture_state,
        capture_task,
        description,
    );
    Ok((id, log_path))
}

fn truncate_cmd_for_desc(command: &str, max: usize) -> String {
    let single_line = command.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.len() <= max {
        return single_line;
    }
    let mut end = max.saturating_sub(1);
    while end > 0 && !single_line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &single_line[..end])
}

#[cfg(test)]
#[path = "bg_convert_boundary_tests.rs"]
mod boundary_tests;

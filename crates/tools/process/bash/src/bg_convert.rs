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
) -> Option<(String, PathBuf)> {
    let data = handle.0.downcast::<TimedOutProcessData>().ok()?;
    let TimedOutProcessData {
        spawned,
        log_path,
        stdout_head_tail,
        stderr_buf,
        drainers,
    } = *data;
    let desc = format!("(auto-bg) {}", truncate_cmd_for_desc(command, 60));
    let id = store.spawn_process_task(
        spawned,
        log_path.clone(),
        stdout_head_tail,
        stderr_buf,
        drainers,
        &desc,
    );
    Some((id, log_path))
}

pub(crate) fn register_spawned(
    store: &Arc<BackgroundTaskStore>,
    handle: ProcessHandle,
    desc: &str,
) -> Option<(String, PathBuf)> {
    let data = handle.0.downcast::<SpawnedBackgroundData>().ok()?;
    let SpawnedBackgroundData {
        spawned,
        log_path,
        head_tail,
        stderr_buf,
        drainers,
    } = *data;
    let id = store.spawn_process_task(
        spawned,
        log_path.clone(),
        head_tail,
        stderr_buf,
        drainers,
        desc,
    );
    Some((id, log_path))
}

fn truncate_cmd_for_desc(cmd: &str, max: usize) -> String {
    let single_line: String = cmd.split_whitespace().collect::<Vec<_>>().join(" ");
    if single_line.len() <= max {
        return single_line;
    }
    let mut end = max.saturating_sub(1);
    while end > 0 && !single_line.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &single_line[..end])
}

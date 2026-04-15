use std::path::Path;

use loopal_context::ContextPipeline;
use loopal_context::middleware::config_refresh::ConfigRefreshMiddleware;
use loopal_context::middleware::file_snapshot::FileSnapshot;

pub(super) fn build_context_pipeline(cwd: &str) -> ContextPipeline {
    let cwd = Path::new(cwd);
    let mut snapshots = Vec::new();

    if let Ok(global) = loopal_config::global_config_dir() {
        snapshots.push(FileSnapshot::load(
            global.join("memory/MEMORY.md"),
            "Global Memory",
        ));
        if let Ok(p) = loopal_config::global_instructions_path() {
            snapshots.push(FileSnapshot::load(p, "Global Instructions"));
        }
        if let Ok(p) = loopal_config::global_local_instructions_path() {
            snapshots.push(FileSnapshot::load(p, "Global Local Instructions"));
        }
        if let Ok(p) = loopal_config::global_settings_path() {
            snapshots.push(FileSnapshot::load(p, "Global Settings"));
        }
    }

    snapshots.push(FileSnapshot::load(
        cwd.join(".loopal/memory/MEMORY.md"),
        "Project Memory",
    ));
    snapshots.push(FileSnapshot::load(
        loopal_config::project_instructions_path(cwd),
        "Project Instructions",
    ));
    snapshots.push(FileSnapshot::load(
        loopal_config::project_local_instructions_path(cwd),
        "Local Instructions",
    ));
    snapshots.push(FileSnapshot::load(
        loopal_config::project_settings_path(cwd),
        "Project Settings",
    ));
    snapshots.push(FileSnapshot::load(
        loopal_config::project_local_settings_path(cwd),
        "Local Settings",
    ));

    let mut pipeline = ContextPipeline::new();
    pipeline.add(Box::new(ConfigRefreshMiddleware::new(snapshots)));
    pipeline
}

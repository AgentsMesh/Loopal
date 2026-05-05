use std::sync::Arc;

use loopal_tool_api::ToolContext;
use loopal_tool_background::BackgroundTaskStore;

fn make_store() -> Arc<BackgroundTaskStore> {
    BackgroundTaskStore::new()
}

fn make_ctx(cwd: &std::path::Path) -> ToolContext {
    let backend = loopal_backend::LocalBackend::new(
        cwd.to_path_buf(),
        None,
        loopal_backend::ResourceLimits::default(),
    );
    ToolContext::new(backend, "test")
}

#[path = "bash_metadata_test.rs"]
mod bash_metadata_test;

#[path = "bash_execution_test.rs"]
mod bash_execution_test;

#[path = "bash_precheck_test.rs"]
mod bash_precheck_test;

#[path = "bash_format_test.rs"]
mod bash_format_test;

#[path = "bash_strategy_test.rs"]
mod bash_strategy_test;

#[path = "streaming_timeout_test.rs"]
mod streaming_timeout_test;

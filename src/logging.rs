use std::path::Path;
use std::time::{Duration, SystemTime};

/// Initialize file-based logging under `~/.loopagent/logs/`.
/// Returns a guard that must be held until the process exits to flush buffered logs.
pub fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = dirs::home_dir()
        .expect("cannot determine home directory")
        .join(".loopagent")
        .join("logs");

    // Ensure the directory exists
    std::fs::create_dir_all(&log_dir).expect("failed to create log directory");

    // Clean up log files older than 7 days
    cleanup_old_logs(&log_dir, 7);

    // Daily-rotating file appender: loopagent.YYYY-MM-DD.log
    let file_appender = tracing_appender::rolling::daily(&log_dir, "loopagent.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Read log level from LOOPAGENT_LOG env var, default to "info"
    let env_filter = std::env::var("LOOPAGENT_LOG").unwrap_or_else(|_| "info".to_string());
    let filter = format!("loopagent={env_filter},loopagent_runtime={env_filter},loopagent_provider={env_filter},loopagent_kernel={env_filter},loopagent_mcp={env_filter},loopagent_tools={env_filter},loopagent_context={env_filter},loopagent_hooks={env_filter},loopagent_storage={env_filter},loopagent_config={env_filter}");

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .init();

    guard
}

/// Remove log files older than `max_age_days` from the log directory.
fn cleanup_old_logs(log_dir: &Path, max_age_days: u64) {
    let cutoff = SystemTime::now() - Duration::from_secs(max_age_days * 24 * 60 * 60);

    if let Ok(entries) = std::fs::read_dir(log_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "log")
                && let Ok(metadata) = path.metadata()
                && let Ok(modified) = metadata.modified()
                && modified < cutoff
            {
                let _ = std::fs::remove_file(&path);
            }
        }
    }
}

/// Initialize file-based logging under `~/.loopal/logs/`.
///
/// Each program run gets its own log file: `loopal-{timestamp}-{pid}.log`.
/// Old log files are cleaned up to keep total directory size bounded.
///
/// Falls back to `{temp_dir}/loopal/logs/` when the primary directory is not
/// writable (e.g. inside Bazel's macOS seatbelt sandbox).
///
/// Returns the log file path and guards — hold all guards until process exit to
/// flush buffered logs and OTel pipelines.
pub fn init_logging(
    telemetry_config: &loopal_config::TelemetryConfig,
) -> (String, loopal_telemetry::TelemetryGuard) {
    let log_dir = pick_writable_log_dir();
    let _ = std::fs::create_dir_all(&log_dir);

    // Housekeep: remove old logs exceeding the retention policy
    crate::log_writer::cleanup_old_logs(&log_dir);

    let writer = crate::log_writer::RotatingFileWriter::new(&log_dir);
    let log_path = writer.current_path();

    let env_filter = std::env::var("LOOPAL_LOG").unwrap_or_else(|_| "info".to_string());
    let filter_str = format!(
        "loopal={env_filter},loopal_runtime={env_filter},\
         loopal_provider={env_filter},loopal_kernel={env_filter},\
         loopal_mcp={env_filter},loopal_tools={env_filter},\
         loopal_context={env_filter},loopal_hooks={env_filter},\
         loopal_storage={env_filter},loopal_config={env_filter}"
    );
    let env_filter = tracing_subscriber::EnvFilter::new(filter_str);

    let guard = loopal_telemetry::init_subscriber(telemetry_config, writer, env_filter);

    tracing::info!(path = %log_path, "logging initialized");

    (log_path, guard)
}

/// Choose a writable log directory: prefer `~/.loopal/logs/`, fall back to
/// `{temp_dir}/loopal/logs/` when the primary is not writable.
fn pick_writable_log_dir() -> std::path::PathBuf {
    let primary = loopal_config::logs_dir();
    if std::fs::create_dir_all(&primary).is_ok() && dir_is_writable(&primary) {
        return primary;
    }
    // Fallback: volatile temp directory (always writable, even in sandboxes)
    let fallback = loopal_config::volatile_dir().join("logs");
    let _ = std::fs::create_dir_all(&fallback);
    fallback
}

/// Probe whether we can actually create files in a directory.
fn dir_is_writable(dir: &std::path::Path) -> bool {
    let probe = dir.join(".write_probe");
    if std::fs::File::create(&probe).is_ok() {
        let _ = std::fs::remove_file(&probe);
        true
    } else {
        false
    }
}

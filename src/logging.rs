/// Initialize file-based logging under `{temp_dir}/loopal/logs/`.
/// Returns a guard that must be held until the process exits to flush buffered logs.
pub fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = loopal_config::logs_dir();

    // Ensure the directory exists
    std::fs::create_dir_all(&log_dir).expect("failed to create log directory");

    // Daily-rotating file appender: loopal.log.YYYY-MM-DD
    let file_appender = tracing_appender::rolling::daily(&log_dir, "loopal.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // Read log level from LOOPAL_LOG env var, default to "info"
    let env_filter = std::env::var("LOOPAL_LOG").unwrap_or_else(|_| "info".to_string());
    let filter = format!("loopal={env_filter},loopal_runtime={env_filter},loopal_provider={env_filter},loopal_kernel={env_filter},loopal_mcp={env_filter},loopal_tools={env_filter},loopal_context={env_filter},loopal_hooks={env_filter},loopal_storage={env_filter},loopal_config={env_filter}");

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(non_blocking)
        .with_ansi(false)
        .with_target(true)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        .init();

    guard
}

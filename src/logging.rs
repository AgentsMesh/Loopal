/// Initialize file-based logging under `{temp_dir}/loopagent/logs/`.
/// Returns a guard that must be held until the process exits to flush buffered logs.
pub fn init_logging() -> tracing_appender::non_blocking::WorkerGuard {
    let log_dir = loopagent_config::logs_dir();

    // Ensure the directory exists
    std::fs::create_dir_all(&log_dir).expect("failed to create log directory");

    // Daily-rotating file appender: loopagent.log.YYYY-MM-DD
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
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::NONE)
        .init();

    guard
}

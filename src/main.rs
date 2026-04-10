mod bootstrap;
mod cli;
mod log_writer;
mod logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Ensure panic backtraces are captured in JoinHandle monitor logs.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        // SAFETY: called before any threads are spawned (single-threaded main entry).
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    }
    // Load telemetry config from settings.json (best-effort; fall back to default
    // so logging always initializes even if config loading fails).
    let cwd = std::env::current_dir().unwrap_or_default();
    let telemetry_config = loopal_config::load_config(&cwd)
        .map(|c| c.settings.telemetry)
        .unwrap_or_default();
    let (_log_path, _otel_guard) = logging::init_logging(&telemetry_config);
    bootstrap::run().await
}

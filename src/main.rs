mod bootstrap;
mod cli;
mod logging;
mod memory_adapter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Ensure panic backtraces are captured in JoinHandle monitor logs.
    if std::env::var_os("RUST_BACKTRACE").is_none() {
        // SAFETY: called before any threads are spawned (single-threaded main entry).
        unsafe { std::env::set_var("RUST_BACKTRACE", "1") };
    }
    let _log_guard = logging::init_logging();
    bootstrap::run().await
}

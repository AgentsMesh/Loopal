mod bootstrap;
mod cli;
mod logging;
mod memory_adapter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log_guard = logging::init_logging();
    bootstrap::run().await
}

mod bootstrap;
mod cli;
mod logging;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _log_guard = logging::init_logging();
    bootstrap::run().await
}

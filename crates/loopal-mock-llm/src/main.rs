use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use loopal_mock_llm_lib::{Scenario, ServerConfig};

#[derive(Parser)]
#[command(name = "loopal-mock-llm")]
struct Args {
    #[arg(long)]
    scenario: PathBuf,
    #[arg(long, default_value = "127.0.0.1:0")]
    bind: SocketAddr,
    #[arg(long, default_value = "loopal-desktop-e2e")]
    api_key: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let scenario = Scenario::from_path(&args.scenario)?;
    loopal_mock_llm_lib::run(ServerConfig {
        bind: args.bind,
        scenario,
        api_key: args.api_key,
    })
    .await
}

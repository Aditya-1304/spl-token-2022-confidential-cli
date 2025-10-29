mod commands;
mod config;
mod crypto;
mod utils;

use anyhow::Result;
use clap::Parser;

#[derive(Parser, Debug)]
#[command(name = "confidential-cli")]
#[command(name = "CLI for SPL Token 2022 Confidential Transfers", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: commands::Commands,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::handle_command(cli.command).await
}

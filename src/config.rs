use anyhow::{Context, Result};
use solana_cli_config::Config;
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::signature::{Keypair, read_keypair_file};
use std::path::PathBuf;

pub struct AppConfig {
    pub rpc_client: RpcClient,
    pub payer: Keypair,
}

impl AppConfig {
    pub fn new() -> Result<Self> {

        let config_file = solana_cli_config::CONFIG_FILE
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Unable to get config file path"))?;
        
        let config = Config::load(config_file)
            .context("Failed to load Solana CLI config")?;

        let rpc_client = RpcClient::new(config.json_rpc_url.clone());

        let payer = read_keypair_file(&config.keypair_path)
            .map_err(|e| anyhow::anyhow!("Failed to read keypair file: {}", e))?;

        Ok(Self { rpc_client, payer })
    }

    pub fn load_keypair(path: &str) -> Result<Keypair> {
        read_keypair_file(path)
            .map_err(|e| anyhow::anyhow!("Failed to read keypair from {}: {}", path, e))
    }
}
use anyhow::{Result, Context};
use solana_sdk::pubkey::Pubkey;
use std::str::FromStr;

pub fn parse_pubkey(s: &str) -> Result<Pubkey> {
    Pubkey::from_str(s).context("Invalid public key format")
}

pub fn format_amount(amount: u64, decimals: u8) -> String {
    let divisor = 10u64.pow(decimals as u32);
    let whole = amount / divisor;
    let fraction = amount % divisor;
    format!("{}.{:0width$}", whole, fraction, width = decimals as usize)
}
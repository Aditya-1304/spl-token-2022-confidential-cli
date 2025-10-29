use anyhow::{Result, Context};
use solana_sdk::{
    signature::Signer,
    transaction::Transaction,
};
use spl_token_2022::extension::StateWithExtensions;
use crate::{config::AppConfig, crypto, utils};

pub async fn execute(account: String, amount: u64) -> Result<()> {
    println!("💰 Depositing to Confidential Account...\n");
    
    let config = AppConfig::new()?;
    let account_pubkey = utils::parse_pubkey(&account)?;
    
    // Fetch account to get mint
    let account_data = config.rpc_client.get_account(&account_pubkey).await?;
    let token_account = StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account_data.data)?;
    
    // Fetch mint to get decimals
    let mint_data = config.rpc_client.get_account(&token_account.base.mint).await?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data.data)?;
    let decimals = mint.base.decimals;
    
    println!("📋 Deposit Details:");
    println!("  Account: {}", account_pubkey);
    println!("  Mint: {}", token_account.base.mint);
    println!("  Amount: {}", utils::format_amount(amount, decimals));
    
    // Derive encryption keys for the owner
    let elgamal_keypair = crypto::derive_elgamal_keypair(&config.payer);
    
    println!("\n🔐 Encryption Info:");
    println!("  ElGamal Public Key: {:?}", elgamal_keypair.pubkey());
    
    // Create deposit instruction
    // This moves tokens from regular balance -> pending balance (encrypted)
    let deposit_ix = spl_token_2022::extension::confidential_transfer::instruction::deposit(
        &spl_token_2022::id(),
        &account_pubkey,
        &token_account.base.mint,
        amount,
        decimals,
        &config.payer.pubkey(),
        &[], // No multisig signers
    )?;
    
    let mut transaction = Transaction::new_with_payer(
        &[deposit_ix],
        Some(&config.payer.pubkey()),
    );
    
    let recent_blockhash = config.rpc_client.get_latest_blockhash().await?;
    transaction.sign(&[&config.payer], recent_blockhash);
    
    println!("\n📤 Sending deposit transaction...");
    let signature = config.rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .context("Failed to deposit")?;
    
    println!("✅ Deposit successful!");
    println!("   Signature: {}", signature);
    
    println!("\n📚 What just happened:");
    println!("   1. {} tokens moved from regular balance", utils::format_amount(amount, decimals));
    println!("   2. Amount encrypted and added to PENDING balance");
    println!("   3. Pending balance uses ElGamal encryption");
    
    println!("\n⚠️  Next Steps:");
    println!("   • Run 'apply-balance' to move pending -> available balance");
    println!("   • Only available balance can be spent in transfers");
    println!("   • Pending balance accumulates incoming transfers");
    
    Ok(())
}
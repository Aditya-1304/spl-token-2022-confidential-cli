use anyhow::{Result, Context};
use solana_sdk::signature::Signer;
use spl_token_2022::{
    extension::{
        confidential_transfer::ConfidentialTransferAccount,
        StateWithExtensions,
    },
    solana_zk_sdk::encryption::pod::elgamal::PodElGamalCiphertext,
};
use crate::{config::AppConfig, crypto, utils};
use spl_token_2022::extension::BaseStateWithExtensions;
use bytemuck::Zeroable;

pub async fn execute(account: String) -> Result<()> {
    println!("💼 Checking Confidential Balance...\n");
    
    let config = AppConfig::new()?;
    let account_pubkey = utils::parse_pubkey(&account)?;
    
    // Fetch account data
    let account_data = config.rpc_client.get_account(&account_pubkey).await
        .context("Failed to fetch account")?;
    
    let token_account = StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account_data.data)?;
    let ct_account = token_account.get_extension::<ConfidentialTransferAccount>()?;
    
    // Fetch mint to get decimals
    let mint_data = config.rpc_client.get_account(&token_account.base.mint).await?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data.data)?;
    let decimals = mint.base.decimals;
    
    println!("📋 Account Information:");
    println!("  Address: {}", account_pubkey);
    println!("  Mint: {}", token_account.base.mint);
    println!("  Owner: {}", token_account.base.owner);
    println!("  Decimals: {}", decimals);
    
    // Derive encryption keys
    let elgamal_keypair = crypto::derive_elgamal_keypair(&config.payer);
    let aes_key = crypto::derive_aes_key(&config.payer);
    
    println!("\n🔐 Encryption Keys:");
    println!("  ElGamal Public Key: {:?}", ct_account.elgamal_pubkey);
    println!("  Approved: {}", bool::from(ct_account.approved));
    
    println!("\n🔓 Decrypting Balances...");
    
    // Decrypt available balance (fast - uses AES)
    let available_balance = if ct_account.available_balance == PodElGamalCiphertext::zeroed() {
        0u64
    } else {
        aes_key.decrypt(&ct_account.decryptable_available_balance.try_into()?)
            .ok_or_else(|| anyhow::anyhow!("Failed to decrypt available balance"))?
    };
    
    println!("  ✅ Available Balance (spendable): {}", 
        utils::format_amount(available_balance, decimals));
    
    // Decrypt pending balance (can be slow)
    println!("\n  Decrypting pending balance...");
    
    let pending_balance_lo = elgamal_keypair.secret().decrypt_u32(&ct_account.pending_balance_lo.try_into()?)
        .ok_or_else(|| anyhow::anyhow!("Failed to decrypt pending balance lo"))?;
    
    let pending_balance_hi = elgamal_keypair.secret().decrypt_u32(&ct_account.pending_balance_hi.try_into()?)
        .ok_or_else(|| anyhow::anyhow!("Failed to decrypt pending balance hi"))?;
    
    // Combine lo (16-bit) and hi (32-bit)
    let pending_balance = (pending_balance_lo as u64) + ((pending_balance_hi as u64) << 16);
    
    println!("  ✅ Pending Balance (incoming): {}", 
        utils::format_amount(pending_balance, decimals));
    
    // Total balance
    let total_balance = available_balance + pending_balance;
    
    println!("\n💰 Balance Summary:");
    println!("  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Available (spendable):  {}", utils::format_amount(available_balance, decimals));
    println!("  Pending (incoming):     {}", utils::format_amount(pending_balance, decimals));
    println!("  ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  Total:                  {}", utils::format_amount(total_balance, decimals));
    
    println!("\n📊 Pending Balance Counter:");
    println!("  Current: {}", u64::from(ct_account.pending_balance_credit_counter));
    println!("  Maximum: {}", u64::from(ct_account.maximum_pending_balance_credit_counter));
    
    if pending_balance > 0 {
        println!("\n⚠️  Action Required:");
        println!("  You have pending balance! Run:");
        println!("  $ confidential-cli apply-balance --account {}", account_pubkey);
        println!("  This will move pending balance -> available balance");
    }
    
    if u64::from(ct_account.pending_balance_credit_counter) > u64::from(ct_account.maximum_pending_balance_credit_counter) / 2 {
        println!("\n⚠️  Warning:");
        println!("  Pending balance counter is over 50% of maximum");
        println!("  Consider running 'apply-balance' soon to prevent overflow");
    }
    
    println!("\n💡 Understanding Your Balance:");
    println!("  • Available Balance: Ready to spend immediately");
    println!("  • Pending Balance: Received transfers not yet applied");
    println!("  • Run 'apply-balance' to merge pending -> available");
    println!("  • All balances are encrypted on-chain");
    println!("  • Only you can decrypt with your ElGamal secret key");
    
    Ok(())
}
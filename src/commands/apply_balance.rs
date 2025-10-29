use anyhow::{Result, Context};
use solana_sdk::{
    signature::Signer,
    transaction::Transaction,
};
use spl_token_2022::{
    extension::{
        confidential_transfer::ConfidentialTransferAccount,
        StateWithExtensions,
    },
    solana_zk_sdk::encryption::pod::elgamal::PodElGamalCiphertext,
};
use spl_token_2022::extension::BaseStateWithExtensions;
use bytemuck::Zeroable;
use crate::{config::AppConfig, crypto, utils};

pub async fn execute(account: String) -> Result<()> {
    println!("🔄 Applying Pending Balance...\n");
    
    let config = AppConfig::new()?;
    let account_pubkey = utils::parse_pubkey(&account)?;
    
    // Fetch account data
    let account_data = config.rpc_client.get_account(&account_pubkey).await?;
    let token_account = StateWithExtensions::<spl_token_2022::state::Account>::unpack(&account_data.data)?;
    let ct_account = token_account.get_extension::<ConfidentialTransferAccount>()?;
    
    // Fetch mint to get decimals
    let mint_data = config.rpc_client.get_account(&token_account.base.mint).await?;
    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data.data)?;
    let decimals = mint.base.decimals;
    
    println!("📋 Account: {}", account_pubkey);
    println!("  Mint: {}", token_account.base.mint);
    
    // Derive encryption keys
    let elgamal_keypair = crypto::derive_elgamal_keypair(&config.payer);
    let aes_key = crypto::derive_aes_key(&config.payer);
    
    println!("\n🔓 Decrypting balances...");
    
    // Decrypt current available balance
    let current_available_balance = if ct_account.available_balance == PodElGamalCiphertext::zeroed() {
        0u64
    } else {
        aes_key.decrypt(&ct_account.decryptable_available_balance.try_into()?)
            .ok_or_else(|| anyhow::anyhow!("Failed to decrypt available balance"))?
    };
    
    println!("  Current Available Balance: {}", utils::format_amount(current_available_balance, decimals));
    
    // Decrypt pending balance (this can be slow if many transfers)
    println!("  Decrypting pending balance (may take time)...");
    
    let pending_balance_lo = elgamal_keypair.secret().decrypt_u32(&ct_account.pending_balance_lo.try_into()?)
        .ok_or_else(|| anyhow::anyhow!("Failed to decrypt pending balance lo"))?;
    
    let pending_balance_hi = elgamal_keypair.secret().decrypt_u32(&ct_account.pending_balance_hi.try_into()?)
        .ok_or_else(|| anyhow::anyhow!("Failed to decrypt pending balance hi"))?;
    
    // Combine lo (16-bit) and hi (32-bit) parts
    let pending_balance = (pending_balance_lo as u64) + ((pending_balance_hi as u64) << 16);
    
    println!("  Pending Balance: {}", utils::format_amount(pending_balance, decimals));
    
    if pending_balance == 0 {
        println!("\n⚠️  No pending balance to apply!");
        return Ok(());
    }
    
    // Calculate new available balance after applying pending
    let new_available_balance = current_available_balance + pending_balance;
    
    println!("\n💡 After applying:");
    println!("  New Available Balance: {}", utils::format_amount(new_available_balance, decimals));
    
    // Encrypt the new available balance
    let new_decryptable_balance = aes_key.encrypt(new_available_balance);
    
    // Create apply pending balance instruction
    // Fix: Convert AeCiphertext to PodAeCiphertext properly
    let pod_decryptable_balance: spl_token_2022::solana_zk_sdk::encryption::pod::auth_encryption::PodAeCiphertext 
        = new_decryptable_balance.into();
    
    let apply_ix = spl_token_2022::extension::confidential_transfer::instruction::apply_pending_balance(
        &spl_token_2022::id(),
        &account_pubkey,
        ct_account.pending_balance_credit_counter.into(),
        &pod_decryptable_balance,
        &config.payer.pubkey(),
        &[], // No multisig signers
    )?;
    
    let mut transaction = Transaction::new_with_payer(
        &[apply_ix],
        Some(&config.payer.pubkey()),
    );
    
    let recent_blockhash = config.rpc_client.get_latest_blockhash().await?;
    transaction.sign(&[&config.payer], recent_blockhash);
    
    println!("\n📤 Sending transaction...");
    let signature = config.rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .context("Failed to apply pending balance")?;
    
    println!("✅ Pending balance applied successfully!");
    println!("   Signature: {}", signature);
    
    println!("\n📚 What just happened:");
    println!("   1. Decrypted your pending balance: {}", utils::format_amount(pending_balance, decimals));
    println!("   2. Added to available balance: {}", utils::format_amount(current_available_balance, decimals));
    println!("   3. New available balance: {}", utils::format_amount(new_available_balance, decimals));
    println!("   4. Pending balance reset to 0");
    println!("   5. Updated decryptable balance for instant access");
    
    println!("\n💡 Key Concepts:");
    println!("   • Pending balance: Accumulated incoming transfers (encrypted)");
    println!("   • Available balance: Spendable balance (encrypted)");
    println!("   • This operation merges pending -> available");
    println!("   • Required before spending newly received tokens");
    
    Ok(())
}
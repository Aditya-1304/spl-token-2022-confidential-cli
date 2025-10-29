use anyhow::{Result, Context};
use solana_sdk::{
    signature::{Keypair, Signer},
    system_instruction,
    transaction::Transaction,
};
use spl_token_2022::{
    extension::{
        confidential_transfer::instruction::initialize_mint,
        ExtensionType,
    },
    state::Mint,
};
use crate::config::AppConfig;

pub async fn execute(authority_path: Option<String>, decimals: u8) -> Result<()> {
    println!("🏭 Creating Confidential Mint...\n");
    
    let config = AppConfig::new()?;
    let mint_keypair = Keypair::new();
    
    let authority = if let Some(path) = authority_path {
        AppConfig::load_keypair(&path)?
    } else {
        // Fixed: Use try_from instead of from_bytes
        Keypair::try_from(&config.payer.to_bytes()[..])
            .map_err(|e| anyhow::anyhow!("Failed to create keypair: {}", e))?
    };
    
    println!("📋 Mint Details:");
    println!("  Address: {}", mint_keypair.pubkey());
    println!("  Authority: {}", authority.pubkey());
    println!("  Decimals: {}", decimals);
    
    // Calculate space needed for mint with confidential transfer extension
    let extensions = vec![ExtensionType::ConfidentialTransferMint];
    let space = ExtensionType::try_calculate_account_len::<Mint>(&extensions)?;
    
    let rent = config.rpc_client
        .get_minimum_balance_for_rent_exemption(space)
        .await?;
    
    println!("\n💰 Rent: {} lamports", rent);
    println!("📦 Account size: {} bytes (with ConfidentialTransfer extension)", space);
    
    // Create account
    let create_account_ix = system_instruction::create_account(
        &config.payer.pubkey(),
        &mint_keypair.pubkey(),
        rent,
        space as u64,
        &spl_token_2022::id(),
    );
    
    // Initialize confidential transfer on mint
    // Fixed: Added the missing `auto_approve_new_accounts` parameter (4th argument)
    let init_confidential_transfer_ix = initialize_mint(
        &spl_token_2022::id(),
        &mint_keypair.pubkey(),
        None,  // Authority (optional)
        true,  // auto_approve_new_accounts - THIS WAS MISSING
        None,  // No auditor ElGamal pubkey
    )?;
    
    // Initialize mint
    let init_mint_ix = spl_token_2022::instruction::initialize_mint(
        &spl_token_2022::id(),
        &mint_keypair.pubkey(),
        &authority.pubkey(),
        Some(&authority.pubkey()),
        decimals,
    )?;
    
    let mut transaction = Transaction::new_with_payer(
        &[create_account_ix, init_confidential_transfer_ix, init_mint_ix],
        Some(&config.payer.pubkey()),
    );
    
    let recent_blockhash = config.rpc_client.get_latest_blockhash().await?;
    transaction.sign(&[&config.payer, &mint_keypair], recent_blockhash);
    
    println!("\n📤 Sending transaction...");
    let signature = config.rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .context("Failed to create mint")?;
    
    println!("✅ Mint created successfully!");
    println!("   Signature: {}", signature);
    println!("\n🔑 Save this mint address: {}", mint_keypair.pubkey());
    
    // Educational output
    println!("\n📚 What just happened:");
    println!("   1. Created a new Token-2022 account");
    println!("   2. Enabled ConfidentialTransferMint extension");
    println!("   3. Initialized the mint with {} decimals", decimals);
    println!("   4. Auto-approve enabled for new confidential accounts");
    println!("\n💡 This mint now supports confidential transfers using:");
    println!("   - Twisted ElGamal encryption for balance privacy");
    println!("   - Zero-knowledge proofs for transfer validity");
    
    Ok(())
}
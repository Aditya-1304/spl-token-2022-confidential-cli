use anyhow::{Result, Context};
use solana_sdk::{
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use spl_token_2022::{
    extension::ExtensionType,
    solana_zk_sdk::{
        encryption::pod::auth_encryption::PodAeCiphertext,
        zk_elgamal_proof_program::{
            proof_data::PubkeyValidityProofData,
            instruction::ProofInstruction,
        },
    },
};
use spl_token_confidential_transfer_proof_extraction::instruction::ProofLocation;
use std::num::NonZero;
use crate::{config::AppConfig, crypto, utils};

pub async fn execute(mint: String, owner_path: Option<String>) -> Result<()> {
    println!("👤 Creating Confidential Token Account...\n");
    
    let config = AppConfig::new()?;
    let mint_pubkey = utils::parse_pubkey(&mint)?;
    
    let owner = if let Some(path) = owner_path {
        AppConfig::load_keypair(&path)?
    } else {
        config.payer.insecure_clone()
    };
    
    let account_keypair = Keypair::new();
    
    println!("📋 Account Details:");
    println!("  Address: {}", account_keypair.pubkey());
    println!("  Owner: {}", owner.pubkey());
    println!("  Mint: {}", mint_pubkey);
    
    // Derive encryption keys - THIS IS CRITICAL
    let elgamal_keypair = crypto::derive_elgamal_keypair(&owner);
    let aes_key = crypto::derive_aes_key(&owner);
    
    crypto::print_encryption_info(&owner);
    
    // Calculate space with confidential transfer extension
    let extensions = vec![ExtensionType::ConfidentialTransferAccount];
    let space = ExtensionType::try_calculate_account_len::<spl_token_2022::state::Account>(&extensions)?;
    
    let rent = config.rpc_client
        .get_minimum_balance_for_rent_exemption(space as usize)
        .await?;
    
    println!("\n💰 Rent: {} lamports", rent);
    println!("📦 Account size: {} bytes", space);
    
    // Step 1: Create the token account
    let create_ix = solana_sdk::system_instruction::create_account(
        &config.payer.pubkey(),
        &account_keypair.pubkey(),
        rent,
        space as u64,
        &spl_token_2022::id(),
    );
    
    // Step 2: Initialize the token account (standard)
    let init_account_ix = spl_token_2022::instruction::initialize_account(
        &spl_token_2022::id(),
        &account_keypair.pubkey(),
        &mint_pubkey,
        &owner.pubkey(),
    )?;
    
    // Step 3: Configure confidential transfers
    println!("\n🔐 Setting up confidential transfer components:");
    println!("   1. Creating encrypted zero balance (decryptable by you)");
    println!("   2. Generating pubkey validity proof");
    println!("   3. Configuring ElGamal public key");
    
    // Create a decryptable zero balance using AES encryption
    let decryptable_zero_balance = aes_key.encrypt(0_u64);
    let pod_decryptable_balance: PodAeCiphertext = decryptable_zero_balance.into();
    
    // Generate pubkey validity proof
    // This proves that your ElGamal public key is well-formed
    let pubkey_validity_proof_data = PubkeyValidityProofData::new(&elgamal_keypair)
        .map_err(|e| anyhow::anyhow!("Failed to create pubkey validity proof: {:?}", e))?;
    
    println!("   ✅ Pubkey validity proof generated");
    println!("      This proves your ElGamal key is valid (ZK proof)");
    
    // Create the proof verification instruction using ProofInstruction enum
    let proof_instruction = ProofInstruction::VerifyPubkeyValidity
        .encode_verify_proof(None, &pubkey_validity_proof_data);
    
    // Configure account with proof location
    // Proof is in the previous instruction (offset -1 from configure_account)
    let proof_location = ProofLocation::InstructionOffset(
        NonZero::new(-1i8).unwrap(),
        &pubkey_validity_proof_data,
    );
    
    // Configure the account with the ElGamal public key
    // This also initializes the confidential transfer extension
    let configure_ixs = spl_token_2022::extension::confidential_transfer::instruction::configure_account(
        &spl_token_2022::id(),
        &account_keypair.pubkey(),
        &mint_pubkey,
        &pod_decryptable_balance,
        u64::MAX,
        &owner.pubkey(),
        &[],
        proof_location,
    )?;
    
    let mut all_instructions = vec![
        create_ix,
        init_account_ix,
        proof_instruction,
    ];
    all_instructions.extend(configure_ixs);
    
    let mut transaction = Transaction::new_with_payer(
        &all_instructions,
        Some(&config.payer.pubkey()),
    );
    
    let recent_blockhash = config.rpc_client.get_latest_blockhash().await?;
    transaction.sign(&[&config.payer, &account_keypair], recent_blockhash);
    
    println!("\n📤 Sending transaction...");
    let signature = config.rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .context("Failed to create confidential account")?;
    
    println!("✅ Confidential token account created successfully!");
    println!("   Signature: {}", signature);
    
    println!("\n🔑 Save this account address: {}", account_keypair.pubkey());
    
    println!("\n📚 What just happened:");
    println!("   1. Created a new Token-2022 account with ConfidentialTransfer extension");
    println!("   2. Initialized the account with standard token functionality");
    println!("   3. Generated a zero-knowledge proof that your ElGamal key is valid");
    println!("   4. Configured the account with:");
    println!("      • Encrypted zero balance (decryptable by you)");
    println!("      • Your ElGamal public key for receiving transfers");
    println!("      • Proof that your key is well-formed");
    
    println!("\n💡 Next Steps:");
    println!("   • Mint some tokens to this account using spl-token CLI");
    println!("   • Deposit tokens to make them confidential");
    println!("   • Use 'balance' command to check your encrypted balances");
    
    Ok(())
}
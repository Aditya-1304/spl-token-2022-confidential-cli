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
    solana_zk_sdk::{
        encryption::{
            pod::elgamal::PodElGamalCiphertext,
            pedersen::PedersenOpening,
        },
        zk_elgamal_proof_program::{
            proof_data::{
                CiphertextCommitmentEqualityProofData,
                BatchedRangeProofU64Data,
            },
            instruction::ProofInstruction,
        },
    },
};
use spl_token_confidential_transfer_proof_extraction::instruction::ProofLocation;
use std::num::NonZero;
use crate::{config::AppConfig, crypto, utils};
use spl_token_2022::extension::BaseStateWithExtensions;
use bytemuck::Zeroable;

pub async fn execute(account: String, amount: u64) -> Result<()> {
    println!("💸 Withdrawing from Confidential Account...\n");
    
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
    
    println!("📋 Withdrawal Details:");
    println!("  Account: {}", account_pubkey);
    println!("  Amount: {}", utils::format_amount(amount, decimals));
    
    // Derive encryption keys
    let elgamal_keypair = crypto::derive_elgamal_keypair(&config.payer);
    let aes_key = crypto::derive_aes_key(&config.payer);
    
    // Decrypt current available balance
    let current_available_balance = if ct_account.available_balance == PodElGamalCiphertext::zeroed() {
        0u64
    } else {
        aes_key.decrypt(&ct_account.decryptable_available_balance.try_into()?)
            .ok_or_else(|| anyhow::anyhow!("Failed to decrypt available balance"))?
    };
    
    println!("\n💰 Current Available Balance: {}", 
        utils::format_amount(current_available_balance, decimals));
    
    if amount > current_available_balance {
        anyhow::bail!("Insufficient balance! Available: {}, Requested: {}", 
            utils::format_amount(current_available_balance, decimals),
            utils::format_amount(amount, decimals));
    }
    
    // Calculate new balance after withdrawal
    let new_available_balance = current_available_balance - amount;
    
    println!("  After Withdrawal: {}", utils::format_amount(new_available_balance, decimals));
    
    println!("\n🔐 Generating withdrawal proofs...");
    
    // 1. Generate CiphertextCommitmentEquality proof
    // This proves the withdrawal amount ciphertext matches the commitment
    let destination_pubkey = elgamal_keypair.pubkey();
    let opening = PedersenOpening::new_rand();
    let withdrawal_ct = destination_pubkey.encrypt_with(amount, &opening);
    
    // Create commitment to the withdrawal amount
    let commitment = spl_token_2022::solana_zk_sdk::encryption::pedersen::Pedersen::with(amount, &opening);
    
    let equality_proof_data = CiphertextCommitmentEqualityProofData::new(
        &elgamal_keypair,
        &withdrawal_ct,
        &commitment,
        &opening,
        amount,
    ).map_err(|e| anyhow::anyhow!("Failed to create equality proof: {:?}", e))?;
    
    println!("  ✅ Ciphertext-commitment equality proof generated");
    
    // 2. Generate Range proof
    // This proves the new balance after withdrawal is a valid u64
    let new_balance_opening = PedersenOpening::new_rand();
    let (new_balance_commitment, _) = spl_token_2022::solana_zk_sdk::encryption::pedersen::Pedersen::new(new_available_balance);
    
    let range_proof_data = BatchedRangeProofU64Data::new(
        vec![&new_balance_commitment],
        vec![new_available_balance],
        vec![64], // bit length
        vec![&new_balance_opening],
    ).map_err(|e| anyhow::anyhow!("Failed to create range proof: {:?}", e))?;
    
    println!("  ✅ Range proof generated");
    
    // Create proof instructions
    let equality_proof_instruction = ProofInstruction::VerifyCiphertextCommitmentEquality
        .encode_verify_proof(None, &equality_proof_data);
    
    let range_proof_instruction = ProofInstruction::VerifyBatchedRangeProofU64
        .encode_verify_proof(None, &range_proof_data);
    
    // Create new decryptable balance
    let new_decryptable_balance = aes_key.encrypt(new_available_balance);
    let pod_decryptable_balance: spl_token_2022::solana_zk_sdk::encryption::pod::auth_encryption::PodAeCiphertext 
        = new_decryptable_balance.into();
    
    // Proof locations: 
    // equality proof is at -2 (two instructions back)
    // range proof is at -1 (one instruction back)
    let equality_proof_location = ProofLocation::InstructionOffset(
        NonZero::new(-2i8).unwrap(),
        &equality_proof_data,
    );
    
    let range_proof_location = ProofLocation::InstructionOffset(
        NonZero::new(-1i8).unwrap(),
        &range_proof_data,
    );
    
    // Create withdraw instruction
    let withdraw_ixs = spl_token_2022::extension::confidential_transfer::instruction::withdraw(
        &spl_token_2022::id(),
        &account_pubkey,
        &token_account.base.mint,
        amount,
        decimals,
        &pod_decryptable_balance,
        &config.payer.pubkey(),
        &[], // No multisig
        equality_proof_location,
        range_proof_location,
    )?;
    
    // Combine instructions: proof1, proof2, withdraw
    let mut all_instructions = vec![
        equality_proof_instruction,
        range_proof_instruction,
    ];
    all_instructions.extend(withdraw_ixs);
    
    let mut transaction = Transaction::new_with_payer(
        &all_instructions,
        Some(&config.payer.pubkey()),
    );
    
    let recent_blockhash = config.rpc_client.get_latest_blockhash().await?;
    transaction.sign(&[&config.payer], recent_blockhash);
    
    println!("\n📤 Sending withdrawal transaction...");
    let signature = config.rpc_client
        .send_and_confirm_transaction(&transaction)
        .await
        .context("Failed to withdraw")?;
    
    println!("✅ Withdrawal successful!");
    println!("   Signature: {}", signature);
    
    println!("\n📚 What just happened:");
    println!("   1. Withdrew {} tokens from encrypted balance", 
        utils::format_amount(amount, decimals));
    println!("   2. Tokens moved from confidential -> regular balance");
    println!("   3. Generated two ZK proofs:");
    println!("      • Equality proof: withdrawal amount is correct");
    println!("      • Range proof: new balance is valid u64");
    println!("   4. Updated available balance: {}", 
        utils::format_amount(new_available_balance, decimals));
    
    println!("\n💡 Key Concepts:");
    println!("   • Withdrawal converts confidential -> non-confidential tokens");
    println!("   • Requires two ZK proofs (equality + range)");
    println!("   • Deducted from available balance only");
    println!("   • Regular balance is now visible on-chain");
    
    Ok(())
}
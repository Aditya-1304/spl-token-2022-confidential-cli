use solana_sdk::signature::Keypair;
use spl_token_2022::solana_zk_sdk::encryption::{
  auth_encryption::AeKey,
  elgamal::{ElGamalKeypair, ElGamalSecretKey},
};

pub fn derive_elgamal_keypair(signer: &Keypair) -> ElGamalKeypair {
  ElGamalKeypair::new_from_signer(
    signer, 
    b""
  ).unwrap()

}

pub fn derive_aes_key(signer: &Keypair) -> AeKey {
  AeKey::new_from_signer(signer, b"").unwrap()
}

pub fn print_encryption_info(keypair: &Keypair) {
  let elgamal_keypair = derive_elgamal_keypair(keypair);

  println!("\n🔐 Encryption Keys Derived:");
  println!("  ElGamal Public Key: {:?}", elgamal_keypair.pubkey());
  println!("  AES-GCM-SIV Key: Derived (32 bytes)");
  println!("\n💡 These keys are deterministically derived from your Solana keypair");
  println!("   - ElGamal: Used for homomorphic encryption (Twisted ElGamal)");
  println!("   - AES: Used for authenticated encryption of opening values");
}
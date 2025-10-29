pub mod create_mint;
pub mod create_account;
pub mod deposit;
pub mod apply_balance;
pub mod transfer;
pub mod withdraw;
pub mod balance;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand, Debug)]
pub enum Commands {
  CreateMint {
    #[arg(short, long)]
    authority: Option<String>,

    #[arg(short, long, default_value = "9")]
    decimals: u8,
  },

  CreateAccount {
    #[arg(short, long)]
    mint: String,

    #[arg(short, long)]
    owner: Option<String>,
  },

  Deposit {
    #[arg(short, long)]
    account: String,

    #[arg(short = 'a', long)]
    amount: u64,
  },

  ApplyBalance {
    #[arg(short, long)]
    account: String,
  },

  ConfidentialTransfer {
    #[arg(short, long)]
    from: String,

    #[arg(short, long)]
    to: String,

    #[arg(short, long)]
    amount: u64,
  },

  Withdraw {
    #[arg(short, long)]
    account: String,

    #[arg(short = 'a', long)]
    amount: u64
  },

  Balance {
    #[arg(short, long)]
    account: String,
  },

}

pub async fn handle_command(command: Commands) -> Result<()> {
    match command {
        Commands::CreateMint { authority, decimals } => {
            create_mint::execute(authority, decimals).await
        }
        Commands::CreateAccount { mint, owner } => {
            create_account::execute(mint, owner).await
        }
        Commands::Deposit { account, amount } => {
            deposit::execute(account, amount).await
        }
        Commands::ApplyBalance { account } => {
            apply_balance::execute(account).await
        }
        Commands::ConfidentialTransfer { from, to, amount } => {
            transfer::execute(from, to, amount).await
        }
        Commands::Withdraw { account, amount } => {
            withdraw::execute(account, amount).await
        }
        Commands::Balance { account } => {
            balance::execute(account).await
        }
    }
}
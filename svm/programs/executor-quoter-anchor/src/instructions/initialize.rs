use anchor_lang::prelude::*;

use crate::state::{Config, CONFIG_SEED};

/// Arguments for the Initialize instruction.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct InitializeArgs {
    /// The address of the quoter (for identification purposes)
    pub quoter_address: Pubkey,
    /// The address authorized to update quotes and chain info
    pub updater_address: Pubkey,
    /// Decimals of the source chain native token (SOL = 9)
    pub src_token_decimals: u8,
    /// Universal address format for payee (32 bytes)
    pub payee_address: [u8; 32],
}

/// Accounts for the Initialize instruction.
#[derive(Accounts)]
pub struct Initialize<'info> {
    /// Payer for account creation
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Config PDA to be created
    #[account(
        init,
        payer = payer,
        space = Config::LEN,
        seeds = [CONFIG_SEED],
        bump,
    )]
    pub config: Account<'info, Config>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Handler for the Initialize instruction.
/// Creates and initializes the Config PDA.
pub fn initialize_handler(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
    let config = &mut ctx.accounts.config;

    config.bump = ctx.bumps.config;
    config.src_token_decimals = args.src_token_decimals;
    config.quoter_address = args.quoter_address;
    config.updater_address = args.updater_address;
    config.payee_address = args.payee_address;

    Ok(())
}

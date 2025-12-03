use anchor_lang::prelude::*;

use crate::error::ExecutorQuoterError;
use crate::state::{ChainInfo, Config, CHAIN_INFO_SEED, CONFIG_SEED};

/// Arguments for the UpdateChainInfo instruction.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateChainInfoArgs {
    /// Destination chain identifier
    pub chain_id: u16,
    /// Whether the chain is enabled for quoting
    pub enabled: bool,
    /// Decimals for gas price on this chain
    pub gas_price_decimals: u8,
    /// Decimals of the native token on this chain
    pub native_decimals: u8,
}

/// Accounts for the UpdateChainInfo instruction.
#[derive(Accounts)]
#[instruction(args: UpdateChainInfoArgs)]
pub struct UpdateChainInfo<'info> {
    /// Payer for account creation if needed
    #[account(mut)]
    pub payer: Signer<'info>,

    /// Updater must match config.updater_address
    pub updater: Signer<'info>,

    /// Config PDA for validation
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
        constraint = config.updater_address == updater.key() @ ExecutorQuoterError::InvalidUpdater,
    )]
    pub config: Account<'info, Config>,

    /// ChainInfo PDA to create/update
    #[account(
        init_if_needed,
        payer = payer,
        space = ChainInfo::LEN,
        seeds = [CHAIN_INFO_SEED, &args.chain_id.to_le_bytes()],
        bump,
    )]
    pub chain_info: Account<'info, ChainInfo>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Handler for the UpdateChainInfo instruction.
/// Creates or updates the ChainInfo PDA for a destination chain.
pub fn update_chain_info_handler(ctx: Context<UpdateChainInfo>, args: UpdateChainInfoArgs) -> Result<()> {
    let chain_info = &mut ctx.accounts.chain_info;

    chain_info.enabled = args.enabled;
    chain_info.chain_id = args.chain_id;
    chain_info.gas_price_decimals = args.gas_price_decimals;
    chain_info.native_decimals = args.native_decimals;
    chain_info.bump = ctx.bumps.chain_info;

    Ok(())
}

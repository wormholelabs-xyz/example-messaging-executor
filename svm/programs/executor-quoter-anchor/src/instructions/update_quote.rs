use anchor_lang::prelude::*;

use crate::error::ExecutorQuoterError;
use crate::state::{Config, QuoteBody, CONFIG_SEED, QUOTE_SEED};

/// Arguments for the UpdateQuote instruction.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct UpdateQuoteArgs {
    /// Destination chain identifier
    pub chain_id: u16,
    /// The USD price, in 10^10, of the destination chain native currency
    pub dst_price: u64,
    /// The USD price, in 10^10, of the source chain native currency
    pub src_price: u64,
    /// The current gas price on the destination chain
    pub dst_gas_price: u64,
    /// The base fee, in source chain native currency, required by the quoter
    pub base_fee: u64,
}

/// Accounts for the UpdateQuote instruction.
#[derive(Accounts)]
#[instruction(args: UpdateQuoteArgs)]
pub struct UpdateQuote<'info> {
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

    /// QuoteBody PDA to create/update
    #[account(
        init_if_needed,
        payer = payer,
        space = QuoteBody::LEN,
        seeds = [QUOTE_SEED, &args.chain_id.to_le_bytes()],
        bump,
    )]
    pub quote_body: Account<'info, QuoteBody>,

    /// System program for account creation
    pub system_program: Program<'info, System>,
}

/// Handler for the UpdateQuote instruction.
/// Creates or updates the QuoteBody PDA for a destination chain.
pub fn update_quote_handler(ctx: Context<UpdateQuote>, args: UpdateQuoteArgs) -> Result<()> {
    let quote_body = &mut ctx.accounts.quote_body;

    quote_body.chain_id = args.chain_id;
    quote_body.bump = ctx.bumps.quote_body;
    quote_body.dst_price = args.dst_price;
    quote_body.src_price = args.src_price;
    quote_body.dst_gas_price = args.dst_gas_price;
    quote_body.base_fee = args.base_fee;

    Ok(())
}

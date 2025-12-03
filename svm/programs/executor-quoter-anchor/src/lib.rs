use anchor_lang::prelude::*;

pub mod error;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

declare_id!("2sDgzrUykBRKVzL3H4dT5wx7oiVAJg22kRVL7mhY1AqM");

#[program]
pub mod executor_quoter_anchor {
    use super::*;

    /// Initialize the ExecutorQuoter config.
    /// Creates the Config PDA with quoter, updater, and payee addresses.
    pub fn initialize(ctx: Context<Initialize>, args: InitializeArgs) -> Result<()> {
        instructions::initialize::initialize_handler(ctx, args)
    }

    /// Update chain info for a destination chain.
    /// Creates or updates the ChainInfo PDA.
    pub fn update_chain_info(
        ctx: Context<UpdateChainInfo>,
        args: UpdateChainInfoArgs,
    ) -> Result<()> {
        instructions::update_chain_info::update_chain_info_handler(ctx, args)
    }

    /// Update quote for a destination chain.
    /// Creates or updates the QuoteBody PDA.
    pub fn update_quote(ctx: Context<UpdateQuote>, args: UpdateQuoteArgs) -> Result<()> {
        instructions::update_quote::update_quote_handler(ctx, args)
    }

    /// Request a quote for cross-chain execution.
    /// Returns the required payment amount via set_return_data.
    pub fn request_quote(ctx: Context<RequestQuote>, args: RequestQuoteArgs) -> Result<()> {
        instructions::get_quote::request_quote_handler(ctx, args)
    }

    /// Request execution quote with full details.
    /// Returns payment, payee address, and quote body via set_return_data.
    pub fn request_execution_quote(
        ctx: Context<RequestExecutionQuote>,
        args: RequestQuoteArgs,
    ) -> Result<()> {
        instructions::get_quote::request_execution_quote_handler(ctx, args)
    }
}

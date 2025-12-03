use anchor_lang::prelude::*;
use anchor_lang::solana_program::program::set_return_data;

use crate::error::ExecutorQuoterError;
use crate::math;
use crate::state::{ChainInfo, Config, QuoteBody, CHAIN_INFO_SEED, CONFIG_SEED, QUOTE_SEED};

/// Relay instruction type constants (matching EVM)
const IX_TYPE_GAS: u8 = 1;
const IX_TYPE_DROP_OFF: u8 = 2;

/// Arguments for RequestQuote and RequestExecutionQuote instructions.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct RequestQuoteArgs {
    /// Destination chain identifier
    pub dst_chain: u16,
    /// Destination address (32 bytes, universal format)
    pub dst_addr: [u8; 32],
    /// Refund address (32 bytes, universal format)
    pub refund_addr: [u8; 32],
    /// Request bytes (variable length)
    pub request_bytes: Vec<u8>,
    /// Relay instructions (variable length)
    pub relay_instructions: Vec<u8>,
}

/// Accounts for the RequestQuote instruction.
#[derive(Accounts)]
#[instruction(args: RequestQuoteArgs)]
pub struct RequestQuote<'info> {
    /// Config PDA
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,

    /// ChainInfo PDA for destination chain
    #[account(
        seeds = [CHAIN_INFO_SEED, &args.dst_chain.to_le_bytes()],
        bump = chain_info.bump,
        constraint = chain_info.enabled @ ExecutorQuoterError::ChainDisabled,
    )]
    pub chain_info: Account<'info, ChainInfo>,

    /// QuoteBody PDA for destination chain
    #[account(
        seeds = [QUOTE_SEED, &args.dst_chain.to_le_bytes()],
        bump = quote_body.bump,
    )]
    pub quote_body: Account<'info, QuoteBody>,
}

/// Accounts for the RequestExecutionQuote instruction.
#[derive(Accounts)]
#[instruction(args: RequestQuoteArgs)]
pub struct RequestExecutionQuote<'info> {
    /// Config PDA
    #[account(
        seeds = [CONFIG_SEED],
        bump = config.bump,
    )]
    pub config: Account<'info, Config>,

    /// ChainInfo PDA for destination chain
    #[account(
        seeds = [CHAIN_INFO_SEED, &args.dst_chain.to_le_bytes()],
        bump = chain_info.bump,
        constraint = chain_info.enabled @ ExecutorQuoterError::ChainDisabled,
    )]
    pub chain_info: Account<'info, ChainInfo>,

    /// QuoteBody PDA for destination chain
    #[account(
        seeds = [QUOTE_SEED, &args.dst_chain.to_le_bytes()],
        bump = quote_body.bump,
    )]
    pub quote_body: Account<'info, QuoteBody>,
}

/// Parses relay instructions to extract total gas limit and msg value.
/// Instruction format:
/// - Type 1 (Gas): 1 byte type + 16 bytes gas_limit + 16 bytes msg_value
/// - Type 2 (DropOff): 1 byte type + 48 bytes (16 msg_value + 32 recipient)
fn parse_relay_instructions(relay_instructions: &[u8]) -> Result<(u128, u128)> {
    let mut offset = 0;
    let mut gas_limit: u128 = 0;
    let mut msg_value: u128 = 0;
    let mut has_drop_off = false;

    while offset < relay_instructions.len() {
        if offset >= relay_instructions.len() {
            return Err(ExecutorQuoterError::InvalidRelayInstructions.into());
        }

        let ix_type = relay_instructions[offset];
        offset += 1;

        match ix_type {
            IX_TYPE_GAS => {
                // Gas instruction: 16 bytes gas_limit + 16 bytes msg_value
                if offset + 32 > relay_instructions.len() {
                    return Err(ExecutorQuoterError::InvalidRelayInstructions.into());
                }

                let mut ix_gas_bytes = [0u8; 16];
                ix_gas_bytes.copy_from_slice(&relay_instructions[offset..offset + 16]);
                let ix_gas_limit = u128::from_be_bytes(ix_gas_bytes);
                offset += 16;

                let mut ix_val_bytes = [0u8; 16];
                ix_val_bytes.copy_from_slice(&relay_instructions[offset..offset + 16]);
                let ix_msg_value = u128::from_be_bytes(ix_val_bytes);
                offset += 16;

                gas_limit = gas_limit
                    .checked_add(ix_gas_limit)
                    .ok_or(ExecutorQuoterError::MathOverflow)?;
                msg_value = msg_value
                    .checked_add(ix_msg_value)
                    .ok_or(ExecutorQuoterError::MathOverflow)?;
            }
            IX_TYPE_DROP_OFF => {
                if has_drop_off {
                    return Err(ExecutorQuoterError::MoreThanOneDropOff.into());
                }
                has_drop_off = true;

                // DropOff instruction: 16 bytes msg_value + 32 bytes recipient
                if offset + 48 > relay_instructions.len() {
                    return Err(ExecutorQuoterError::InvalidRelayInstructions.into());
                }

                let mut ix_val_bytes = [0u8; 16];
                ix_val_bytes.copy_from_slice(&relay_instructions[offset..offset + 16]);
                let ix_msg_value = u128::from_be_bytes(ix_val_bytes);
                offset += 48; // Skip msg_value (16) + recipient (32)

                msg_value = msg_value
                    .checked_add(ix_msg_value)
                    .ok_or(ExecutorQuoterError::MathOverflow)?;
            }
            _ => {
                return Err(ExecutorQuoterError::UnsupportedInstruction.into());
            }
        }
    }

    Ok((gas_limit, msg_value))
}

/// Handler for the RequestQuote instruction.
/// Returns the required payment amount via set_return_data.
pub fn request_quote_handler(ctx: Context<RequestQuote>, args: RequestQuoteArgs) -> Result<()> {
    let chain_info = &ctx.accounts.chain_info;
    let quote_body = &ctx.accounts.quote_body;

    // Parse relay instructions
    let (gas_limit, msg_value) = parse_relay_instructions(&args.relay_instructions)?;

    // Calculate quote using U256 math
    let required_payment = math::estimate_quote(
        quote_body.base_fee,
        quote_body.src_price,
        quote_body.dst_price,
        quote_body.dst_gas_price,
        chain_info.gas_price_decimals,
        chain_info.native_decimals,
        gas_limit,
        msg_value,
    )?;

    // Return the quote as big-endian U256 (32 bytes) via set_return_data.
    // Clients can read this via simulateTransaction or CPI callers via get_return_data.
    set_return_data(&required_payment.to_be_bytes());

    Ok(())
}

/// Handler for the RequestExecutionQuote instruction.
/// Returns the required payment, payee address, and quote body via set_return_data.
pub fn request_execution_quote_handler(
    ctx: Context<RequestExecutionQuote>,
    args: RequestQuoteArgs,
) -> Result<()> {
    let config = &ctx.accounts.config;
    let chain_info = &ctx.accounts.chain_info;
    let quote_body = &ctx.accounts.quote_body;

    // Parse relay instructions
    let (gas_limit, msg_value) = parse_relay_instructions(&args.relay_instructions)?;

    // Calculate quote using U256 math
    let required_payment = math::estimate_quote(
        quote_body.base_fee,
        quote_body.src_price,
        quote_body.dst_price,
        quote_body.dst_gas_price,
        chain_info.gas_price_decimals,
        chain_info.native_decimals,
        gas_limit,
        msg_value,
    )?;

    // Return data layout (96 bytes, matching EVM return values):
    // - bytes 0-31: required_payment (U256, big-endian)
    // - bytes 32-63: payee_address (32 bytes)
    // - bytes 64-95: quote_body (32 bytes, EQ01 format)
    let mut return_data = [0u8; 96];
    return_data[0..32].copy_from_slice(&required_payment.to_be_bytes());
    return_data[32..64].copy_from_slice(&config.payee_address);
    return_data[64..96].copy_from_slice(&quote_body.to_bytes32());

    set_return_data(&return_data);

    Ok(())
}

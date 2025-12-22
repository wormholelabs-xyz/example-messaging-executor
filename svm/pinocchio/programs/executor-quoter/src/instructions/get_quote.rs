use executor_requests::parse_relay_instructions;
use pinocchio::{
    account_info::AccountInfo, cpi::set_return_data, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::{
    error::{relay_parse_error_to_program_error, ExecutorQuoterError},
    math,
    state::{load_account, ChainInfo, QuoteBody},
    PAYEE_ADDRESS,
};

/// Process RequestQuote instruction.
/// Returns the required payment amount for cross-chain execution.
///
/// Accounts:
/// 0. `[]` config - Config PDA
/// 1. `[]` chain_info - ChainInfo PDA for destination chain
/// 2. `[]` quote_body - QuoteBody PDA for destination chain
///
/// Instruction data layout:
/// - dst_chain: u16 (offset 0)
/// - dst_addr: [u8; 32] (offset 2)
/// - refund_addr: [u8; 32] (offset 34)
/// - request_bytes_len: u32 (offset 66)
/// - request_bytes: [u8; request_bytes_len] (offset 70)
/// - relay_instructions_len: u32 (offset 70 + request_bytes_len)
/// - relay_instructions: [u8; relay_instructions_len]
pub fn process_request_quote(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Parse accounts
    let [_config, chain_info_account, quote_body_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load accounts (discriminator checked inside load_account)
    let chain_info = load_account::<ChainInfo>(chain_info_account, program_id)?;
    if !chain_info.is_enabled() {
        return Err(ExecutorQuoterError::ChainDisabled.into());
    }

    let quote_body = load_account::<QuoteBody>(quote_body_account, program_id)?;

    // Parse instruction data to get relay_instructions
    // Skip: dst_chain (2) + dst_addr (32) + refund_addr (32) = 66 bytes
    if data.len() < 70 {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    // Skip request_bytes
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&data[66..70]);
    let request_bytes_len = u32::from_le_bytes(len_bytes) as usize;
    let relay_start = 70 + request_bytes_len;

    if data.len() < relay_start + 4 {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    let mut relay_len_bytes = [0u8; 4];
    relay_len_bytes.copy_from_slice(&data[relay_start..relay_start + 4]);
    let relay_instructions_len = u32::from_le_bytes(relay_len_bytes) as usize;

    let relay_data_start = relay_start + 4;
    if data.len() < relay_data_start + relay_instructions_len {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    let relay_instructions = &data[relay_data_start..relay_data_start + relay_instructions_len];

    // Parse relay instructions
    let (gas_limit, msg_value) =
        parse_relay_instructions(relay_instructions).map_err(relay_parse_error_to_program_error)?;

    // Calculate quote - returns u64 in SVM native decimals (lamports)
    let required_payment = math::estimate_quote(&quote_body, &chain_info, gas_limit, msg_value)?;

    // Return the quote as u64 (8 bytes, big-endian) via set_return_data.
    set_return_data(&required_payment.to_be_bytes());

    Ok(())
}

/// Process RequestExecutionQuote instruction.
/// Returns the required payment, payee address, and quote body.
///
/// Accounts:
/// 0. `[]` config - Config PDA
/// 1. `[]` chain_info - ChainInfo PDA for destination chain
/// 2. `[]` quote_body - QuoteBody PDA for destination chain
/// 3. `[]` event_cpi - Account for event CPI (unused in this implementation, but required for interface compatibility)
pub fn process_request_execution_quote(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Parse accounts - _config and event_cpi are required but unused in this implementation.
    // Future quoter implementations may use them.
    let [_config, chain_info_account, quote_body_account, _event_cpi] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load accounts (discriminator checked inside load_account)
    let chain_info = load_account::<ChainInfo>(chain_info_account, program_id)?;
    if !chain_info.is_enabled() {
        return Err(ExecutorQuoterError::ChainDisabled.into());
    }

    let quote_body = load_account::<QuoteBody>(quote_body_account, program_id)?;

    // Parse instruction data to get relay_instructions
    if data.len() < 70 {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&data[66..70]);
    let request_bytes_len = u32::from_le_bytes(len_bytes) as usize;
    let relay_start = 70 + request_bytes_len;

    if data.len() < relay_start + 4 {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    let mut relay_len_bytes = [0u8; 4];
    relay_len_bytes.copy_from_slice(&data[relay_start..relay_start + 4]);
    let relay_instructions_len = u32::from_le_bytes(relay_len_bytes) as usize;

    let relay_data_start = relay_start + 4;
    if data.len() < relay_data_start + relay_instructions_len {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    let relay_instructions = &data[relay_data_start..relay_data_start + relay_instructions_len];

    // Parse relay instructions
    let (gas_limit, msg_value) =
        parse_relay_instructions(relay_instructions).map_err(relay_parse_error_to_program_error)?;

    // Calculate quote - returns u64 in SVM native decimals (lamports)
    let required_payment = math::estimate_quote(&quote_body, &chain_info, gas_limit, msg_value)?;

    // Return data layout (72 bytes, all big-endian):
    // - bytes 0-7: required_payment (u64)
    // - bytes 8-39: payee_address (32 bytes)
    // - bytes 40-71: quote_body (32 bytes, EQ01 format)
    let mut return_data = [0u8; 72];
    return_data[0..8].copy_from_slice(&required_payment.to_be_bytes());
    return_data[8..40].copy_from_slice(&PAYEE_ADDRESS);
    return_data[40..72].copy_from_slice(&quote_body.to_bytes32());

    set_return_data(&return_data);

    Ok(())
}

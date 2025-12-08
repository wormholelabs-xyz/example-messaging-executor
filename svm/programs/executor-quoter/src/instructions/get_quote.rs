use pinocchio::{
    account_info::AccountInfo, cpi::set_return_data, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::{
    error::ExecutorQuoterError,
    math,
    state::{load_account, ChainInfo, Config, QuoteBody},
};

/// Relay instruction type constants (matching EVM)
const IX_TYPE_GAS: u8 = 1;
const IX_TYPE_DROP_OFF: u8 = 2;

/// Parses relay instructions to extract total gas limit and msg value.
/// Instruction format:
/// - Type 1 (Gas): 1 byte type + 16 bytes gas_limit + 16 bytes msg_value
/// - Type 2 (DropOff): 1 byte type + 48 bytes (16 msg_value + 32 recipient)
fn parse_relay_instructions(relay_instructions: &[u8]) -> Result<(u128, u128), ProgramError> {
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
    let [config_account, chain_info_account, quote_body_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load accounts (discriminator checked inside load_account)
    let _config = load_account::<Config>(config_account, program_id)?;

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
    let (gas_limit, msg_value) = parse_relay_instructions(relay_instructions)?;

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
    // Parse accounts - event_cpi is required but unused in this implementation.
    // Future quoter implementations may use it for logging via CPI.
    let [config_account, chain_info_account, quote_body_account, _event_cpi] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load accounts (discriminator checked inside load_account)
    let config = load_account::<Config>(config_account, program_id)?;

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
    let (gas_limit, msg_value) = parse_relay_instructions(relay_instructions)?;

    // Calculate quote - returns u64 in SVM native decimals (lamports)
    let required_payment = math::estimate_quote(&quote_body, &chain_info, gas_limit, msg_value)?;

    // Return data layout (72 bytes, all big-endian):
    // - bytes 0-7: required_payment (u64)
    // - bytes 8-39: payee_address (32 bytes)
    // - bytes 40-71: quote_body (32 bytes, EQ01 format)
    let mut return_data = [0u8; 72];
    return_data[0..8].copy_from_slice(&required_payment.to_be_bytes());
    return_data[8..40].copy_from_slice(&config.payee_address);
    return_data[40..72].copy_from_slice(&quote_body.to_bytes32());

    set_return_data(&return_data);

    Ok(())
}

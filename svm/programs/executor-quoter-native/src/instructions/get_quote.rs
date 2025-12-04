use solana_program::{
    account_info::AccountInfo, entrypoint::ProgramResult, program::set_return_data,
    program_error::ProgramError, pubkey::Pubkey,
};

use crate::{
    error::ExecutorQuoterError,
    math,
    state::{
        pack_quote_body_to_bytes32, read_bytes32, read_u64_le, read_u8, validate_account,
        CHAIN_INFO_DISCRIMINATOR, CHAIN_INFO_ENABLED_OFFSET, CHAIN_INFO_GAS_PRICE_DECIMALS_OFFSET,
        CHAIN_INFO_LEN, CHAIN_INFO_NATIVE_DECIMALS_OFFSET, CONFIG_DISCRIMINATOR, CONFIG_LEN,
        CONFIG_PAYEE_ADDRESS_OFFSET, QUOTE_BODY_BASE_FEE_OFFSET, QUOTE_BODY_DISCRIMINATOR,
        QUOTE_BODY_DST_GAS_PRICE_OFFSET, QUOTE_BODY_DST_PRICE_OFFSET, QUOTE_BODY_LEN,
        QUOTE_BODY_SRC_PRICE_OFFSET,
    },
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

    // Validate config account
    validate_account(config_account, program_id, CONFIG_DISCRIMINATOR, CONFIG_LEN)?;

    // Validate chain_info account and check if enabled
    validate_account(
        chain_info_account,
        program_id,
        CHAIN_INFO_DISCRIMINATOR,
        CHAIN_INFO_LEN,
    )?;

    // Validate quote_body account
    validate_account(
        quote_body_account,
        program_id,
        QUOTE_BODY_DISCRIMINATOR,
        QUOTE_BODY_LEN,
    )?;

    // Read chain_info fields
    let (gas_price_decimals, native_decimals) = {
        let chain_info_data = chain_info_account.try_borrow_data()?;

        // Check if chain is enabled
        if read_u8(&chain_info_data, CHAIN_INFO_ENABLED_OFFSET) == 0 {
            return Err(ExecutorQuoterError::ChainDisabled.into());
        }

        (
            read_u8(&chain_info_data, CHAIN_INFO_GAS_PRICE_DECIMALS_OFFSET),
            read_u8(&chain_info_data, CHAIN_INFO_NATIVE_DECIMALS_OFFSET),
        )
    };

    // Read quote_body fields
    let (base_fee, src_price, dst_price, dst_gas_price) = {
        let quote_body_data = quote_body_account.try_borrow_data()?;
        (
            read_u64_le(&quote_body_data, QUOTE_BODY_BASE_FEE_OFFSET),
            read_u64_le(&quote_body_data, QUOTE_BODY_SRC_PRICE_OFFSET),
            read_u64_le(&quote_body_data, QUOTE_BODY_DST_PRICE_OFFSET),
            read_u64_le(&quote_body_data, QUOTE_BODY_DST_GAS_PRICE_OFFSET),
        )
    };

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
    let required_payment = math::estimate_quote(
        base_fee,
        src_price,
        dst_price,
        dst_gas_price,
        gas_price_decimals,
        native_decimals,
        gas_limit,
        msg_value,
    )?;

    // Return the quote as u64 (8 bytes, big-endian) via set_return_data.
    set_return_data(&required_payment.to_be_bytes());

    Ok(())
}

/// Process RequestExecutionQuote instruction.
/// Returns the required payment, payee address, and quote body.
///
/// Accounts: Same as RequestQuote
pub fn process_request_execution_quote(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // Parse accounts
    let [config_account, chain_info_account, quote_body_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate config account
    validate_account(config_account, program_id, CONFIG_DISCRIMINATOR, CONFIG_LEN)?;

    // Validate chain_info account
    validate_account(
        chain_info_account,
        program_id,
        CHAIN_INFO_DISCRIMINATOR,
        CHAIN_INFO_LEN,
    )?;

    // Validate quote_body account
    validate_account(
        quote_body_account,
        program_id,
        QUOTE_BODY_DISCRIMINATOR,
        QUOTE_BODY_LEN,
    )?;

    // Read chain_info fields
    let (gas_price_decimals, native_decimals) = {
        let chain_info_data = chain_info_account.try_borrow_data()?;

        // Check if chain is enabled
        if read_u8(&chain_info_data, CHAIN_INFO_ENABLED_OFFSET) == 0 {
            return Err(ExecutorQuoterError::ChainDisabled.into());
        }

        (
            read_u8(&chain_info_data, CHAIN_INFO_GAS_PRICE_DECIMALS_OFFSET),
            read_u8(&chain_info_data, CHAIN_INFO_NATIVE_DECIMALS_OFFSET),
        )
    };

    // Read quote_body fields and config payee_address
    let (base_fee, src_price, dst_price, dst_gas_price) = {
        let quote_body_data = quote_body_account.try_borrow_data()?;
        (
            read_u64_le(&quote_body_data, QUOTE_BODY_BASE_FEE_OFFSET),
            read_u64_le(&quote_body_data, QUOTE_BODY_SRC_PRICE_OFFSET),
            read_u64_le(&quote_body_data, QUOTE_BODY_DST_PRICE_OFFSET),
            read_u64_le(&quote_body_data, QUOTE_BODY_DST_GAS_PRICE_OFFSET),
        )
    };

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
    let required_payment = math::estimate_quote(
        base_fee,
        src_price,
        dst_price,
        dst_gas_price,
        gas_price_decimals,
        native_decimals,
        gas_limit,
        msg_value,
    )?;

    // Return data layout (72 bytes, all big-endian):
    // - bytes 0-7: required_payment (u64)
    // - bytes 8-39: payee_address (32 bytes)
    // - bytes 40-71: quote_body (32 bytes, EQ01 format)
    let mut return_data = [0u8; 72];
    return_data[0..8].copy_from_slice(&required_payment.to_be_bytes());

    // Read payee_address from config
    {
        let config_data = config_account.try_borrow_data()?;
        let payee_address = read_bytes32(&config_data, CONFIG_PAYEE_ADDRESS_OFFSET);
        return_data[8..40].copy_from_slice(payee_address);
    }

    // Pack quote body to bytes32 (EQ01 format)
    let quote_body_bytes32 = pack_quote_body_to_bytes32(base_fee, dst_gas_price, src_price, dst_price);
    return_data[40..72].copy_from_slice(&quote_body_bytes32);

    set_return_data(&return_data);

    Ok(())
}

//! RequestExecution instruction for the ExecutorQuoterRouter.
//!
//! The main execution flow:
//! 1. CPI to quoter's RequestExecutionQuote to get payment/payee/quote body
//! 2. Handle payment (transfer to payee, refund excess)
//! 3. Construct EQ02 signed quote
//! 4. CPI to Executor's request_for_execution

use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};
use pinocchio_system::instructions::Transfer;

use super::quoter_cpi::{make_executor_request_for_execution_ix, make_quoter_request_execution_quote_ix};
use super::serialization::make_signed_quote_eq02;
use crate::{
    error::ExecutorQuoterRouterError,
    state::{load_account, Config, QuoterRegistration, EXPIRY_TIME_MAX},
};

/// RequestExecution instruction.
///
/// Accounts:
/// 0. `[signer, writable]` payer - Pays for execution
/// 1. `[]` config - Router config PDA
/// 2. `[]` quoter_registration - QuoterRegistration PDA for the quoter
/// 3. `[]` quoter_program - The quoter implementation program
/// 4. `[]` executor_program - The executor program to CPI into
/// 5. `[writable]` payee - Receives the payment
/// 6. `[writable]` refund_addr - Receives any excess payment
/// 7. `[]` system_program
/// 8-11. `[]` quoter accounts: quoter_config, chain_info, quote_body, event_cpi
///
/// Instruction data layout:
/// - quoter_address: [u8; 20] (20 bytes)
/// - amount: u64 le (8 bytes) - The amount being paid (msg.value equivalent)
/// - dst_chain: u16 le (2 bytes)
/// - dst_addr: [u8; 32] (32 bytes)
/// - refund_addr_bytes: [u8; 32] (32 bytes) - Universal address for refund
/// - request_bytes_len: u32 le (4 bytes)
/// - request_bytes: [u8; request_bytes_len]
/// - relay_instructions_len: u32 le (4 bytes)
/// - relay_instructions: [u8; relay_instructions_len]
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Minimum data: 20 + 8 + 2 + 32 + 32 + 4 + 4 = 102 bytes
    if data.len() < 102 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Parse quoter_address
    let mut quoter_address = [0u8; 20];
    quoter_address.copy_from_slice(&data[0..20]);

    // Parse amount
    let mut amount_bytes = [0u8; 8];
    amount_bytes.copy_from_slice(&data[20..28]);
    let amount = u64::from_le_bytes(amount_bytes);

    // Parse dst_chain
    let mut dst_chain_bytes = [0u8; 2];
    dst_chain_bytes.copy_from_slice(&data[28..30]);
    let dst_chain = u16::from_le_bytes(dst_chain_bytes);

    // Parse dst_addr
    let mut dst_addr = [0u8; 32];
    dst_addr.copy_from_slice(&data[30..62]);

    // Parse refund_addr_bytes
    let mut refund_addr_bytes = [0u8; 32];
    refund_addr_bytes.copy_from_slice(&data[62..94]);

    // Parse request_bytes
    let mut request_bytes_len_arr = [0u8; 4];
    request_bytes_len_arr.copy_from_slice(&data[94..98]);
    let request_bytes_len = u32::from_le_bytes(request_bytes_len_arr) as usize;

    if data.len() < 98 + request_bytes_len + 4 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    let request_bytes = &data[98..98 + request_bytes_len];

    // Parse relay_instructions
    let relay_start = 98 + request_bytes_len;
    let mut relay_len_arr = [0u8; 4];
    relay_len_arr.copy_from_slice(&data[relay_start..relay_start + 4]);
    let relay_instructions_len = u32::from_le_bytes(relay_len_arr) as usize;

    if data.len() < relay_start + 4 + relay_instructions_len {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    let relay_instructions = &data[relay_start + 4..relay_start + 4 + relay_instructions_len];

    // Parse accounts
    let [payer, config_account, quoter_registration_account, quoter_program, _executor_program, payee, refund_account, system_program, quoter_config, quoter_chain_info, quoter_quote_body, event_cpi] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Verify payer is signer
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load config
    let config = load_account::<Config>(config_account, program_id)?;

    // Load and verify quoter registration
    let registration = load_account::<QuoterRegistration>(quoter_registration_account, program_id)?;

    if registration.quoter_address != quoter_address {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    if quoter_program.key() != &registration.implementation_program_id {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    // Step 1: CPI to quoter's RequestExecutionQuote
    let quoter_ix_data = make_quoter_request_execution_quote_ix(
        dst_chain,
        &dst_addr,
        &refund_addr_bytes,
        request_bytes,
        relay_instructions,
    );

    let cpi_instruction = pinocchio::instruction::Instruction {
        program_id: &registration.implementation_program_id,
        accounts: &[
            pinocchio::instruction::AccountMeta {
                pubkey: quoter_config.key(),
                is_signer: false,
                is_writable: false,
            },
            pinocchio::instruction::AccountMeta {
                pubkey: quoter_chain_info.key(),
                is_signer: false,
                is_writable: false,
            },
            pinocchio::instruction::AccountMeta {
                pubkey: quoter_quote_body.key(),
                is_signer: false,
                is_writable: false,
            },
            pinocchio::instruction::AccountMeta {
                pubkey: event_cpi.key(),
                is_signer: false,
                is_writable: false,
            },
        ],
        data: &quoter_ix_data,
    };

    pinocchio::cpi::invoke(
        &cpi_instruction,
        &[quoter_config, quoter_chain_info, quoter_quote_body, event_cpi],
    )?;

    // Get return data from quoter: (required_payment, payee_address, quote_body)
    // Layout: 8 bytes payment + 32 bytes payee + 32 bytes quote_body = 72 bytes
    let return_data = pinocchio::cpi::get_return_data()
        .ok_or(ExecutorQuoterRouterError::InvalidReturnData)?;

    if return_data.len() < 72 {
        return Err(ExecutorQuoterRouterError::InvalidReturnData.into());
    }

    let mut required_payment_bytes = [0u8; 8];
    required_payment_bytes.copy_from_slice(&return_data[0..8]);
    let required_payment = u64::from_be_bytes(required_payment_bytes);

    let mut payee_address = [0u8; 32];
    payee_address.copy_from_slice(&return_data[8..40]);

    let mut quote_body = [0u8; 32];
    quote_body.copy_from_slice(&return_data[40..72]);

    // Step 2: Handle payment
    if amount < required_payment {
        return Err(ExecutorQuoterRouterError::Underpaid.into());
    }

    // Transfer required payment to payee
    Transfer {
        from: payer,
        to: payee,
        lamports: required_payment,
    }
    .invoke()?;

    // Refund excess (no check since check done above)
    let excess = amount - required_payment;

    if excess > 0 {
        Transfer {
            from: payer,
            to: refund_account,
            lamports: excess,
        }
        .invoke()?;
    }

    // Step 3: Construct EQ02 signed quote
    let signed_quote = make_signed_quote_eq02(
        &quoter_address,
        &payee_address,
        config.our_chain,
        dst_chain,
        EXPIRY_TIME_MAX, // Use max expiry for on-chain quotes
        &quote_body,
    );

    // Step 4: CPI to Executor's request_for_execution
    // Build the instruction data for Anchor's request_for_execution
    let executor_ix_data =
        make_executor_request_for_execution_ix(amount, dst_chain, &dst_addr, &refund_addr_bytes, &signed_quote, request_bytes, relay_instructions);

    let executor_cpi_instruction = pinocchio::instruction::Instruction {
        program_id: &config.executor_program_id,
        accounts: &[
            // payer - signer, writable
            pinocchio::instruction::AccountMeta {
                pubkey: payer.key(),
                is_signer: true,
                is_writable: true,
            },
            // payee - writable (receives the payment)
            pinocchio::instruction::AccountMeta {
                pubkey: payee.key(),
                is_signer: false,
                is_writable: true,
            },
            // system_program
            pinocchio::instruction::AccountMeta {
                pubkey: &pinocchio_system::ID,
                is_signer: false,
                is_writable: false,
            },
        ],
        data: &executor_ix_data,
    };

    pinocchio::cpi::invoke(
        &executor_cpi_instruction,
        &[payer, payee, system_program],
    )?;

    Ok(())
}

//! QuoteExecution instruction for the ExecutorQuoterRouter.
//!
//! Gets a quote from a registered quoter via CPI.

use pinocchio::{
    account_info::AccountInfo, cpi::set_return_data, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use super::quoter_cpi::make_quoter_request_quote_ix;
use crate::{
    error::ExecutorQuoterRouterError,
    state::{load_account, QuoterRegistration},
};

/// QuoteExecution instruction.
///
/// Accounts:
/// 0. `[]` quoter_registration - QuoterRegistration PDA for the quoter
/// 1. `[]` quoter_program - The quoter implementation program
/// 2-4. `[]` quoter accounts: config, chain_info, quote_body (passed to quoter)
///
/// Instruction data layout:
/// - quoter_address: [u8; 20] (20 bytes) - The quoter address to look up
/// - dst_chain: u16 le (2 bytes)
/// - dst_addr: [u8; 32] (32 bytes)
/// - refund_addr: [u8; 32] (32 bytes)
/// - request_bytes_len: u32 le (4 bytes)
/// - request_bytes: [u8; request_bytes_len]
/// - relay_instructions_len: u32 le (4 bytes)
/// - relay_instructions: [u8; relay_instructions_len]
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Minimum data: 20 + 2 + 32 + 32 + 4 + 4 = 94 bytes
    if data.len() < 94 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Parse quoter_address from instruction data
    let mut quoter_address = [0u8; 20];
    quoter_address.copy_from_slice(&data[0..20]);

    // Parse accounts
    let [quoter_registration_account, quoter_program, quoter_config, quoter_chain_info, quoter_quote_body] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load and verify quoter registration
    let registration = load_account::<QuoterRegistration>(quoter_registration_account, program_id)?;

    // Verify the quoter address matches
    if registration.quoter_address != quoter_address {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    // Verify the quoter program matches the registration
    if quoter_program.key() != &registration.implementation_program_id {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    // Build CPI instruction data for RequestQuote
    // Skip the quoter_address (20 bytes) and use the rest as the quoter instruction data
    let quoter_ix_data = make_quoter_request_quote_ix(
        u16::from_le_bytes([data[20], data[21]]), // dst_chain
        data[22..54].try_into().unwrap(),         // dst_addr
        data[54..86].try_into().unwrap(),         // refund_addr
        &data[90..90 + u32::from_le_bytes([data[86], data[87], data[88], data[89]]) as usize], // request_bytes
        &data[90
            + u32::from_le_bytes([data[86], data[87], data[88], data[89]]) as usize
            + 4..], // relay_instructions (simplified, should parse length properly)
    );

    // Invoke quoter's RequestQuote
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
        ],
        data: &quoter_ix_data,
    };

    pinocchio::cpi::invoke(&cpi_instruction, &[quoter_config, quoter_chain_info, quoter_quote_body])?;

    // Get return data from quoter and forward it
    // The quoter returns the required payment as u64 (8 bytes, big-endian)
    // We just forward it as-is
    let return_data = pinocchio::cpi::get_return_data()
        .ok_or(ExecutorQuoterRouterError::InvalidReturnData)?;
    set_return_data(return_data.as_slice());

    Ok(())
}

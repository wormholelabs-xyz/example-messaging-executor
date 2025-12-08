//! QuoteExecution instruction for the ExecutorQuoterRouter.
//!
//! Gets a quote from a registered quoter via CPI.

use bytemuck::{Pod, Zeroable};
use pinocchio::{
    account_info::AccountInfo, cpi::set_return_data, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use super::quoter_cpi::make_quoter_request_quote_ix;
use crate::{
    error::ExecutorQuoterRouterError,
    state::{load_account, QuoterRegistration},
};

/// Fixed header for QuoteExecution instruction data.
/// Variable-length fields (request_bytes, relay_instructions) follow after.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct QuoteExecutionHeader {
    pub quoter_address: [u8; 20],
    pub dst_chain: u16,
    pub dst_addr: [u8; 32],
    pub refund_addr: [u8; 32],
    pub _padding: [u8; 2],
    pub request_bytes_len: u32,
}

impl QuoteExecutionHeader {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

/// QuoteExecution instruction.
///
/// Accounts:
/// 0. `[]` quoter_registration - QuoterRegistration PDA for the quoter
/// 1. `[]` quoter_program - The quoter implementation program
///    2-4. `[]` quoter accounts: config, chain_info, quote_body (passed to quoter)
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Minimum: header (90) + relay_instructions_len (4) = 94 bytes
    if data.len() < QuoteExecutionHeader::LEN + 4 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Parse fixed header
    let header: QuoteExecutionHeader =
        bytemuck::try_pod_read_unaligned(&data[..QuoteExecutionHeader::LEN])
            .map_err(|_| ExecutorQuoterRouterError::InvalidInstructionData)?;

    let quoter_address = header.quoter_address;

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

    // Parse variable-length fields after header
    let request_bytes_len = header.request_bytes_len as usize;
    let request_bytes_start = QuoteExecutionHeader::LEN;
    let request_bytes_end = request_bytes_start + request_bytes_len;

    if data.len() < request_bytes_end + 4 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    let request_bytes = &data[request_bytes_start..request_bytes_end];

    let relay_len_start = request_bytes_end;
    let relay_instructions_len = u32::from_le_bytes(
        data[relay_len_start..relay_len_start + 4]
            .try_into()
            .map_err(|_| ExecutorQuoterRouterError::InvalidInstructionData)?,
    ) as usize;

    let relay_instructions_start = relay_len_start + 4;
    let relay_instructions_end = relay_instructions_start + relay_instructions_len;

    if data.len() < relay_instructions_end {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    let relay_instructions = &data[relay_instructions_start..relay_instructions_end];

    // Build CPI instruction data for RequestQuote
    let quoter_ix_data = make_quoter_request_quote_ix(
        header.dst_chain,
        &header.dst_addr,
        &header.refund_addr,
        request_bytes,
        relay_instructions,
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

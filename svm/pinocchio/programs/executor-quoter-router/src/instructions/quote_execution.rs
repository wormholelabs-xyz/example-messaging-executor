//! QuoteExecution instruction for the ExecutorQuoterRouter.
//!
//! Gets a quote from a registered quoter via CPI.
//!
//! Input layout (zero-copy optimized):
//! - bytes 0-19: quoter_address (20 bytes, for registration lookup)
//! - bytes 20+: quoter CPI data (passed directly, includes 8-byte discriminator)
//!
//! The client must set bytes 20-27 to the quoter's RequestQuote discriminator
//! (Anchor-compatible: byte 0 = 2, bytes 1-7 = padding zeros).

use pinocchio::{
    account_info::AccountInfo, cpi::set_return_data, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::{
    error::ExecutorQuoterRouterError,
    state::{load_account, QuoterRegistration},
};

/// Offset where quoter CPI data starts (after quoter_address).
const QUOTER_CPI_OFFSET: usize = 20;

/// Expected discriminator for quoter RequestQuote instruction (8 bytes, Anchor-compatible).
/// Byte 0 = instruction ID (2), bytes 1-7 = padding (zeros).
const EXPECTED_QUOTER_DISCRIMINATOR: [u8; 8] = [2, 0, 0, 0, 0, 0, 0, 0];

/// Minimum instruction data size:
/// quoter_address (20) + discriminator (8) + dst_chain (2) + dst_addr (32) +
/// refund_addr (32) + request_bytes_len (4) + relay_instructions_len (4) = 102
const MIN_DATA_LEN: usize = 102;

/// QuoteExecution instruction.
///
/// Accounts:
/// 0. `[]` quoter_registration - QuoterRegistration PDA for the quoter
/// 1. `[]` quoter_program - The quoter implementation program
/// 2-4. `[]` quoter accounts: config, chain_info, quote_body (passed to quoter)
///
/// Instruction Data Layout (minimum 102 bytes):
/// ```text
/// Offset  Size  Field
/// ------  ----  -----
/// 0       20    quoter_address - For registration lookup
///
/// --- Quoter CPI data (passed directly to quoter) ---
/// 20      8     discriminator - Must be [2, 0, 0, 0, 0, 0, 0, 0]
/// 28      2     dst_chain (u16 LE) - Destination chain ID
/// 30      32    dst_addr - Destination address
/// 62      32    refund_addr - Refund address
/// 94      4     request_bytes_len (u32 LE)
/// 98      var   request_bytes
/// var     4     relay_instructions_len (u32 LE)
/// var     var   relay_instructions
/// ```

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() < MIN_DATA_LEN {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Parse quoter_address (bytes 0-19)
    let quoter_address: [u8; 20] = data[0..20]
        .try_into()
        .map_err(|_| ExecutorQuoterRouterError::InvalidInstructionData)?;

    // Parse accounts
    let [quoter_registration_account, quoter_program, quoter_config, quoter_chain_info, quoter_quote_body] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load and verify quoter registration
    let registration = load_account::<QuoterRegistration>(quoter_registration_account, program_id)?;

    if registration.quoter_address != quoter_address {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    if quoter_program.key() != &registration.implementation_program_id {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    // Validate CPI data bounds before passing
    // CPI data layout: discriminator (8) + dst_chain (2) + dst_addr (32) + refund_addr (32) +
    // request_bytes_len (4) + request_bytes + relay_instructions_len (4) + relay_instructions
    let cpi_data = &data[QUOTER_CPI_OFFSET..];

    // Minimum CPI data: discriminator (8) + dst_chain (2) + dst_addr (32) + refund_addr (32) +
    // request_bytes_len (4) + relay_instructions_len (4) = 82 bytes
    if cpi_data.len() < 82 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Validate discriminator matches expected RequestQuote instruction (8-byte Anchor-compatible)
    if cpi_data[0..8] != EXPECTED_QUOTER_DISCRIMINATOR {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Validate request_bytes bounds (offset 74 = 8 discriminator + 2 dst_chain + 32 dst_addr + 32 refund_addr)
    let request_bytes_len = u32::from_le_bytes(
        cpi_data[74..78]
            .try_into()
            .map_err(|_| ExecutorQuoterRouterError::InvalidInstructionData)?,
    ) as usize;

    let relay_len_offset = 78 + request_bytes_len;
    if cpi_data.len() < relay_len_offset + 4 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Validate relay_instructions bounds
    let relay_instructions_len = u32::from_le_bytes(
        cpi_data[relay_len_offset..relay_len_offset + 4]
            .try_into()
            .map_err(|_| ExecutorQuoterRouterError::InvalidInstructionData)?,
    ) as usize;

    let expected_len = relay_len_offset + 4 + relay_instructions_len;
    if cpi_data.len() < expected_len {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Zero-copy: use the CPI data slice directly (includes discriminator set by client)
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
        data: cpi_data,
    };

    pinocchio::cpi::invoke(
        &cpi_instruction,
        &[quoter_config, quoter_chain_info, quoter_quote_body],
    )?;

    // Get return data from quoter and forward it
    let return_data =
        pinocchio::cpi::get_return_data().ok_or(ExecutorQuoterRouterError::InvalidReturnData)?;
    set_return_data(return_data.as_slice());

    Ok(())
}

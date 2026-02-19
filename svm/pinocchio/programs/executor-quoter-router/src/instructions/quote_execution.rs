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

use core::mem;

use pinocchio::{
    account_info::AccountInfo, cpi::set_return_data, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::{
    error::ExecutorQuoterRouterError,
    state::{load_account, QuoterRegistration},
};

/// EVM-style quoter address (first 20 bytes of a universal address).
type QuoterAddress = [u8; 20];

/// Expected discriminator for quoter RequestQuote instruction (8 bytes, Anchor-compatible).
/// Byte 0 = instruction ID (2), bytes 1-7 = padding (zeros).
const EXPECTED_QUOTER_DISCRIMINATOR: [u8; 8] = [2, 0, 0, 0, 0, 0, 0, 0];

/// Minimum instruction data size:
/// quoter_address + discriminator + dst_chain + dst_addr + refund_addr +
/// request_bytes_len + relay_instructions_len
const MIN_DATA_LEN: usize = mem::size_of::<QuoterAddress>()
    + EXPECTED_QUOTER_DISCRIMINATOR.len()
    + mem::size_of::<u16>() // dst_chain
    + 32 // dst_addr
    + 32 // refund_addr
    + mem::size_of::<u32>() // request_bytes_len
    + mem::size_of::<u32>(); // relay_instructions_len

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

    // Extract quoter_address; remaining slice is CPI data passed directly to quoter.
    // unwrap: safe because MIN_DATA_LEN >= size_of::<QuoterAddress>().
    let (quoter_address, cpi_data): (&QuoterAddress, _) = data.split_first_chunk().unwrap();

    // Parse accounts
    let [quoter_registration_account, quoter_program, quoter_config, quoter_chain_info, quoter_quote_body] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Load and verify quoter registration
    let registration = load_account::<QuoterRegistration>(quoter_registration_account, program_id)?;

    if registration.quoter_address != *quoter_address {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    if quoter_program.key() != &registration.implementation_program_id {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    // Validate CPI data structure using split_first_chunk for each fixed-size field.
    // unwrap calls are safe: MIN_DATA_LEN guarantees sufficient bytes for all fixed fields.
    let (discriminator, rest): (&[u8; 8], _) = cpi_data.split_first_chunk().unwrap();

    if *discriminator != EXPECTED_QUOTER_DISCRIMINATOR {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    let (_, rest): (&[u8; 2], _) = rest.split_first_chunk().unwrap(); // dst_chain
    let (_, rest): (&[u8; 32], _) = rest.split_first_chunk().unwrap(); // dst_addr
    let (_, rest): (&[u8; 32], _) = rest.split_first_chunk().unwrap(); // refund_addr
    let (req_len_bytes, rest): (&[u8; 4], _) = rest.split_first_chunk().unwrap();
    let request_bytes_len = u32::from_le_bytes(*req_len_bytes) as usize;

    // Validate variable-length request_bytes
    if rest.len() < request_bytes_len {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }
    let rest = &rest[request_bytes_len..];

    // Validate relay_instructions_len and relay_instructions
    let (relay_len_bytes, rest): (&[u8; 4], _) = rest.split_first_chunk().ok_or(
        ProgramError::from(ExecutorQuoterRouterError::InvalidInstructionData),
    )?;
    let relay_instructions_len = u32::from_le_bytes(*relay_len_bytes) as usize;

    if rest.len() < relay_instructions_len {
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

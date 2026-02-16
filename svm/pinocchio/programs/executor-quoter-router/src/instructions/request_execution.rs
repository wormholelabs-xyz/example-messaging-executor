//! RequestExecution instruction for the ExecutorQuoterRouter.
//!
//! The main execution flow:
//! 1. CPI to quoter's RequestExecutionQuote to get payment/payee/quote body
//! 2. Handle payment (transfer to payee, refund excess)
//! 3. Construct EQ02 signed quote
//! 4. CPI to Executor's request_for_execution
//!
//! Input layout (zero-copy optimized):
//! - bytes 0-7: amount (u64 LE, payment amount)
//! - bytes 8-27: quoter_address (20 bytes, for registration lookup)
//! - bytes 28+: quoter CPI data (passed directly, includes 8-byte discriminator)
//!
//! The client must set bytes 28-35 to the quoter's RequestExecutionQuote discriminator
//! (Anchor-compatible: byte 0 = 3, bytes 1-7 = padding zeros).

use core::mem;

use pinocchio::{
    account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

use super::executor_cpi::make_executor_request_for_execution_ix;
use super::serialization::make_signed_quote_eq02;
use crate::{
    error::ExecutorQuoterRouterError,
    state::{load_account, QuoterRegistration, EXPIRY_TIME_MAX},
    EXECUTOR_PROGRAM_ID, OUR_CHAIN,
};

/// EVM-style quoter address (first 20 bytes of a universal address).
type QuoterAddress = [u8; 20];

/// Expected discriminator for quoter RequestExecutionQuote instruction (8 bytes, Anchor-compatible).
/// Byte 0 = instruction ID (3), bytes 1-7 = padding (zeros).
const EXPECTED_QUOTER_DISCRIMINATOR: [u8; 8] = [3, 0, 0, 0, 0, 0, 0, 0];

/// Minimum instruction data size:
/// amount + quoter_address + discriminator + dst_chain + dst_addr + refund_addr +
/// request_bytes_len + relay_instructions_len
const MIN_DATA_LEN: usize = mem::size_of::<u64>() // amount
    + mem::size_of::<QuoterAddress>()
    + EXPECTED_QUOTER_DISCRIMINATOR.len()
    + mem::size_of::<u16>() // dst_chain
    + 32 // dst_addr
    + 32 // refund_addr
    + mem::size_of::<u32>() // request_bytes_len
    + mem::size_of::<u32>(); // relay_instructions_len

/// RequestExecution instruction.
///
/// Accounts:
/// 0. `[signer, writable]` payer - Pays for execution
/// 1. `[]` _config - reserved for integrator use
/// 2. `[]` quoter_registration - QuoterRegistration PDA for the quoter
/// 3. `[]` quoter_program - The quoter implementation program
/// 4. `[]` executor_program - The executor program to CPI into
/// 5. `[writable]` payee - Receives the payment
/// 6. `[writable]` refund_addr - Receives any excess payment
/// 7. `[]` system_program
/// 8-11. `[]` quoter accounts: quoter_config, chain_info, quote_body, event_cpi
///
/// Instruction Data Layout (minimum 110 bytes):
/// ```text
/// Offset  Size  Field
/// ------  ----  -----
/// 0       8     amount (u64 LE) - Payment amount
/// 8       20    quoter_address - For registration lookup
///
/// --- Quoter CPI data (passed directly to quoter) ---
/// 28      8     discriminator - Must be [3, 0, 0, 0, 0, 0, 0, 0]
/// 36      2     dst_chain (u16 LE) - Destination chain ID
/// 38      32    dst_addr - Destination address
/// 70      32    refund_addr - Refund address
/// 102     4     request_bytes_len (u32 LE)
/// 106     var   request_bytes
/// var     4     relay_instructions_len (u32 LE)
/// var     var   relay_instructions
/// ```
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    if data.len() < MIN_DATA_LEN {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Extract amount and quoter_address; remaining slice is CPI data passed directly to quoter.
    // unwrap: safe because MIN_DATA_LEN >= size_of::<u64>() + size_of::<QuoterAddress>().
    let (amount_bytes, rest): (&[u8; 8], _) = data.split_first_chunk().unwrap();
    let amount = u64::from_le_bytes(*amount_bytes);

    let (quoter_address, cpi_data): (&QuoterAddress, _) = rest.split_first_chunk().unwrap();

    // Validate CPI data structure using split_first_chunk for each fixed-size field.
    // unwrap calls are safe: MIN_DATA_LEN guarantees sufficient bytes for all fixed fields.
    let (discriminator, rest): (&[u8; 8], _) = cpi_data.split_first_chunk().unwrap();

    if *discriminator != EXPECTED_QUOTER_DISCRIMINATOR {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    let (dst_chain_bytes, rest): (&[u8; 2], _) = rest.split_first_chunk().unwrap();
    let dst_chain = u16::from_le_bytes(*dst_chain_bytes);

    let (dst_addr, rest): (&[u8; 32], _) = rest.split_first_chunk().unwrap();
    let (refund_addr_bytes, rest): (&[u8; 32], _) = rest.split_first_chunk().unwrap();

    let (req_len_bytes, rest): (&[u8; 4], _) = rest.split_first_chunk().unwrap();
    let request_bytes_len = u32::from_le_bytes(*req_len_bytes) as usize;

    // Validate variable-length request_bytes
    if rest.len() < request_bytes_len {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }
    let (request_bytes, rest) = rest.split_at(request_bytes_len);

    // Validate relay_instructions_len and relay_instructions
    let (relay_len_bytes, rest): (&[u8; 4], _) = rest
        .split_first_chunk()
        .ok_or(ProgramError::from(
            ExecutorQuoterRouterError::InvalidInstructionData,
        ))?;
    let relay_instructions_len = u32::from_le_bytes(*relay_len_bytes) as usize;

    if rest.len() < relay_instructions_len {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }
    let relay_instructions = &rest[..relay_instructions_len];

    // Parse accounts
    let [payer, _config, quoter_registration_account, quoter_program, _executor_program, payee, _refund_account, system_program, quoter_config, quoter_chain_info, quoter_quote_body, event_cpi] =
        accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Verify payer is signer
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load and verify quoter registration
    let registration = load_account::<QuoterRegistration>(quoter_registration_account, program_id)?;

    if registration.quoter_address != *quoter_address {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    if quoter_program.key() != &registration.implementation_program_id {
        return Err(ExecutorQuoterRouterError::QuoterNotRegistered.into());
    }

    // Step 1: CPI to quoter's RequestExecutionQuote (zero-copy)
    let quoter_cpi_instruction = pinocchio::instruction::Instruction {
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
        data: cpi_data,
    };

    pinocchio::cpi::invoke(
        &quoter_cpi_instruction,
        &[
            quoter_config,
            quoter_chain_info,
            quoter_quote_body,
            event_cpi,
        ],
    )?;

    // Get return data from quoter: (required_payment, payee_address, quote_body)
    // Layout: 8 bytes payment + 32 bytes payee + 32 bytes quote_body = 72 bytes
    let return_data =
        pinocchio::cpi::get_return_data().ok_or(ExecutorQuoterRouterError::InvalidReturnData)?;

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

    // Step 3: Construct EQ02 signed quote
    let signed_quote = make_signed_quote_eq02(
        quoter_address,
        &payee_address,
        OUR_CHAIN,
        dst_chain,
        EXPIRY_TIME_MAX,
        &quote_body,
    );

    // Step 4: CPI to Executor's request_for_execution
    // Note: This still requires allocation due to signed_quote being constructed on-chain
    let executor_ix_data = make_executor_request_for_execution_ix(
        amount,
        dst_chain,
        dst_addr,
        refund_addr_bytes,
        &signed_quote,
        request_bytes,
        relay_instructions,
    );

    let executor_cpi_instruction = pinocchio::instruction::Instruction {
        program_id: &EXECUTOR_PROGRAM_ID,
        accounts: &[
            pinocchio::instruction::AccountMeta {
                pubkey: payer.key(),
                is_signer: true,
                is_writable: true,
            },
            pinocchio::instruction::AccountMeta {
                pubkey: payee.key(),
                is_signer: false,
                is_writable: true,
            },
            pinocchio::instruction::AccountMeta {
                pubkey: &pinocchio_system::ID,
                is_signer: false,
                is_writable: false,
            },
        ],
        data: &executor_ix_data,
    };

    pinocchio::cpi::invoke(&executor_cpi_instruction, &[payer, payee, system_program])?;

    Ok(())
}

//! RequestExecution instruction for the ExecutorQuoterRouter.
//!
//! The main execution flow:
//! 1. CPI to quoter's RequestExecutionQuote to get payment/payee/quote body
//! 2. Handle payment (transfer to payee, refund excess)
//! 3. Construct EQ02 signed quote
//! 4. CPI to Executor's request_for_execution

use bytemuck::{Pod, Zeroable};
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

/// Fixed header for RequestExecution instruction data.
/// Fields are ordered for optimal alignment (amount first as u64).
/// Variable-length fields (request_bytes, relay_instructions) follow after.
/// Total size: 104 bytes (padded for u64 alignment).
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct RequestExecutionHeader {
    pub amount: u64,
    pub quoter_address: [u8; 20],
    pub dst_chain: u16,
    pub dst_addr: [u8; 32],
    pub refund_addr: [u8; 32],
    pub _padding1: [u8; 2],
    pub request_bytes_len: u32,
    pub _padding2: [u8; 4],
}

impl RequestExecutionHeader {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

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
///    8-11. `[]` quoter accounts: quoter_config, chain_info, quote_body, event_cpi
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Minimum: header (98) + relay_instructions_len (4) = 102 bytes
    if data.len() < RequestExecutionHeader::LEN + 4 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Parse fixed header
    let header: RequestExecutionHeader =
        bytemuck::try_pod_read_unaligned(&data[..RequestExecutionHeader::LEN])
            .map_err(|_| ExecutorQuoterRouterError::InvalidInstructionData)?;

    let amount = header.amount;
    let quoter_address = header.quoter_address;
    let dst_chain = header.dst_chain;
    let dst_addr = header.dst_addr;
    let refund_addr_bytes = header.refund_addr;

    // Parse variable-length fields after header
    let request_bytes_len = header.request_bytes_len as usize;
    let request_bytes_start = RequestExecutionHeader::LEN;
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

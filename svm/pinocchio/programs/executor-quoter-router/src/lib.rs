#![no_std]

use pinocchio::{
    account_info::AccountInfo, default_allocator, nostd_panic_handler, program_entrypoint,
    program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

program_entrypoint!(process_instruction);
default_allocator!();
nostd_panic_handler!();

pub mod error;
pub mod instructions;
pub mod state;

use instructions::*;

/// Program ID - replace with actual deployed address
pub static ID: Pubkey = [
    0x0e, 0xf8, 0xc4, 0xd6, 0x7b, 0x42, 0x89, 0xd6, 0x3e, 0xf0, 0x63, 0x1b, 0x5d, 0x0c, 0x39, 0x18,
    0x2e, 0x8c, 0x9a, 0x4f, 0x7f, 0x9d, 0x8a, 0x3b, 0x6c, 0x5e, 0x4d, 0x3c, 0x2b, 0x1a, 0x09, 0xf9,
];

// =============================================================================
// Hardcoded Configuration
// TODO: Replace with env variables at build time
// =============================================================================

/// Solana Wormhole chain ID
pub const OUR_CHAIN: u16 = 1;

/// Executor program ID: execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV
pub const EXECUTOR_PROGRAM_ID: Pubkey = [
    0x09, 0xb9, 0x69, 0x71, 0x58, 0x3b, 0x59, 0x03,
    0xe0, 0x28, 0x1d, 0xa9, 0x65, 0x48, 0xd5, 0xd2,
    0x3c, 0x65, 0x1f, 0x7a, 0x9c, 0xcd, 0xe3, 0xea,
    0xd5, 0x2b, 0x42, 0xf6, 0xb7, 0xda, 0xc2, 0xd2,
];

/// Instruction discriminators
#[repr(u8)]
pub enum Instruction {
    /// Register or update a quoter's implementation mapping
    /// Accounts: [payer, sender, _config, quoter_registration, system_program]
    UpdateQuoterContract = 0,

    /// Get a quote from a registered quoter (read-only CPI)
    /// Accounts: [_config, quoter_registration, quoter_program, ...quoter_accounts]
    QuoteExecution = 1,

    /// Request execution through the router
    /// Accounts: [payer, _config, quoter_registration, quoter_program, executor_program, payee, refund_addr, system_program, ...quoter_accounts]
    RequestExecution = 2,
}

impl TryFrom<u8> for Instruction {
    type Error = ProgramError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Instruction::UpdateQuoterContract),
            1 => Ok(Instruction::QuoteExecution),
            2 => Ok(Instruction::RequestExecution),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    if instruction_data.is_empty() {
        return Err(ProgramError::InvalidInstructionData);
    }

    let (instruction_discriminator, data) = instruction_data.split_at(1);
    let instruction = Instruction::try_from(instruction_discriminator[0])?;

    match instruction {
        Instruction::UpdateQuoterContract => {
            update_quoter_contract::process(program_id, accounts, data)
        }
        Instruction::QuoteExecution => quote_execution::process(program_id, accounts, data),
        Instruction::RequestExecution => request_execution::process(program_id, accounts, data),
    }
}

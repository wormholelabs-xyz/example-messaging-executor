#![no_std]

use pinocchio::{
    account_info::AccountInfo, default_allocator, nostd_panic_handler, program_entrypoint,
    program_error::ProgramError, pubkey::Pubkey, ProgramResult,
};

// Use program_entrypoint to declare the entrypoint
// The MAX_TX_ACCOUNTS default handles any account count
program_entrypoint!(process_instruction);
default_allocator!();
nostd_panic_handler!();

pub mod error;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

/// Program ID - replace with actual deployed address
pub static ID: Pubkey = [
    0x0d, 0xf8, 0xc4, 0xd6, 0x7b, 0x42, 0x89, 0xd6,
    0x3e, 0xf0, 0x63, 0x1b, 0x5d, 0x0c, 0x39, 0x18,
    0x2e, 0x8c, 0x9a, 0x4f, 0x7f, 0x9d, 0x8a, 0x3b,
    0x6c, 0x5e, 0x4d, 0x3c, 0x2b, 0x1a, 0x09, 0xf8,
];

// =============================================================================
// Build-time Configuration
// Set via environment variables: QUOTER_UPDATER_PUBKEY, QUOTER_PAYEE_PUBKEY
// =============================================================================

/// Decimals of the source chain native token (SOL = 9)
pub const SRC_TOKEN_DECIMALS: u8 = 9;

/// Address authorized to update quotes and chain info.
/// Set at build time via QUOTER_UPDATER_PUBKEY env var (base58 pubkey).
pub const UPDATER_ADDRESS: Pubkey = include!(concat!(env!("OUT_DIR"), "/updater_address.rs"));

/// Universal address for the payee (receives execution fees).
/// Set at build time via QUOTER_PAYEE_PUBKEY env var (base58 pubkey).
pub const PAYEE_ADDRESS: [u8; 32] = include!(concat!(env!("OUT_DIR"), "/payee_address.rs"));

/// Instruction discriminators
#[repr(u8)]
pub enum Instruction {
    /// Update chain info for a destination chain
    /// Accounts: [payer, updater, _config, chain_info, system_program]
    UpdateChainInfo = 0,
    /// Update quote for a destination chain
    /// Accounts: [payer, updater, _config, quote_body, system_program]
    UpdateQuote = 1,
    /// Request a quote for cross-chain execution
    /// Accounts: [_config, chain_info, quote_body]
    RequestQuote = 2,
    /// Request execution quote with full details
    /// Accounts: [_config, chain_info, quote_body, event_cpi]
    RequestExecutionQuote = 3,
}

impl TryFrom<u8> for Instruction {
    type Error = ProgramError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Instruction::UpdateChainInfo),
            1 => Ok(Instruction::UpdateQuote),
            2 => Ok(Instruction::RequestQuote),
            3 => Ok(Instruction::RequestExecutionQuote),
            _ => Err(ProgramError::InvalidInstructionData),
        }
    }
}

pub fn process_instruction(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    instruction_data: &[u8],
) -> ProgramResult {
    // Anchor-compatible: 8-byte discriminator (byte 0 = instruction ID, bytes 1-7 ignored)
    if instruction_data.len() < 8 {
        return Err(ProgramError::InvalidInstructionData);
    }

    let (instruction_discriminator, data) = instruction_data.split_at(8);
    let instruction = Instruction::try_from(instruction_discriminator[0])?;

    match instruction {
        Instruction::UpdateChainInfo => update_chain_info::process(program_id, accounts, data),
        Instruction::UpdateQuote => update_quote::process(program_id, accounts, data),
        Instruction::RequestQuote => get_quote::process_request_quote(program_id, accounts, data),
        Instruction::RequestExecutionQuote => {
            get_quote::process_request_execution_quote(program_id, accounts, data)
        }
    }
}

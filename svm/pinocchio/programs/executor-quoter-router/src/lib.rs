#![no_std]

use pinocchio::{
    account_info::AccountInfo, default_allocator, program_entrypoint, program_error::ProgramError,
    pubkey::Pubkey, ProgramResult,
};
use shank::ShankInstruction;

program_entrypoint!(process_instruction);
default_allocator!();

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
// Build-time Configuration
// Set via environment variables: ROUTER_CHAIN_ID, ROUTER_EXECUTOR_PROGRAM_ID
// Defaults to Solana devnet values if unset.
// =============================================================================

/// Wormhole chain ID for the deployment chain.
/// Set at build time via ROUTER_CHAIN_ID env var (u16). Default: 1 (Solana).
pub const OUR_CHAIN: u16 = include!(concat!(env!("OUT_DIR"), "/our_chain.rs"));

/// Executor program ID.
/// Set at build time via ROUTER_EXECUTOR_PROGRAM_ID env var (base58 pubkey).
/// Default: execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV.
pub const EXECUTOR_PROGRAM_ID: Pubkey =
    include!(concat!(env!("OUT_DIR"), "/executor_program_id.rs"));

/// Instruction discriminators
#[derive(ShankInstruction)]
#[repr(u8)]
pub enum Instruction {
    /// Register or update a quoter's implementation mapping
    #[account(
        0,
        writable,
        signer,
        name = "payer",
        desc = "Pays for account creation"
    )]
    #[account(
        1,
        signer,
        name = "sender",
        desc = "Must match universal_sender_address in governance message"
    )]
    #[account(
        2,
        name = "config",
        desc = "Program config (reserved for integrator use)"
    )]
    #[account(
        3,
        writable,
        name = "quoter_registration",
        desc = "QuoterRegistration PDA ['quoter_registration', quoter_address]"
    )]
    #[account(4, name = "system_program", desc = "System Program")]
    UpdateQuoterContract = 0,

    /// Get a quote from a registered quoter (read-only CPI)
    #[account(
        0,
        name = "quoter_registration",
        desc = "QuoterRegistration PDA for the quoter"
    )]
    #[account(1, name = "quoter_program", desc = "Quoter implementation program")]
    #[account(
        2,
        name = "quoter_config",
        desc = "Quoter config account (passed to quoter CPI)"
    )]
    #[account(
        3,
        name = "quoter_chain_info",
        desc = "Quoter chain info account (passed to quoter CPI)"
    )]
    #[account(
        4,
        name = "quoter_quote_body",
        desc = "Quoter quote body account (passed to quoter CPI)"
    )]
    QuoteExecution = 1,

    /// Request execution through the router
    #[account(0, writable, signer, name = "payer", desc = "Pays for execution")]
    #[account(
        1,
        name = "config",
        desc = "Program config (reserved for integrator use)"
    )]
    #[account(
        2,
        name = "quoter_registration",
        desc = "QuoterRegistration PDA for the quoter"
    )]
    #[account(3, name = "quoter_program", desc = "Quoter implementation program")]
    #[account(4, name = "executor_program", desc = "Executor program to CPI into")]
    #[account(5, writable, name = "payee", desc = "Receives execution payment")]
    #[account(
        6,
        writable,
        name = "refund_addr",
        desc = "Receives excess payment refund"
    )]
    #[account(7, name = "system_program", desc = "System Program")]
    #[account(
        8,
        name = "quoter_config",
        desc = "Quoter config account (passed to quoter CPI)"
    )]
    #[account(
        9,
        name = "quoter_chain_info",
        desc = "Quoter chain info account (passed to quoter CPI)"
    )]
    #[account(
        10,
        name = "quoter_quote_body",
        desc = "Quoter quote body account (passed to quoter CPI)"
    )]
    #[account(
        11,
        name = "event_cpi",
        desc = "Event CPI account (passed to quoter CPI)"
    )]
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

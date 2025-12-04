use solana_program::{
    account_info::AccountInfo, declare_id, entrypoint, entrypoint::ProgramResult,
    program_error::ProgramError, pubkey::Pubkey,
};

declare_id!("9CFzEuwodz3UhfZeDpBqpRJGpnYLbBcADMTUEmXvGu42");

entrypoint!(process_instruction);

pub mod error;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

/// Instruction discriminators
#[repr(u8)]
pub enum Instruction {
    /// Initialize the ExecutorQuoter config
    /// Accounts: [payer, config, system_program]
    Initialize = 0,
    /// Update chain info for a destination chain
    /// Accounts: [payer, updater, config, chain_info, system_program]
    UpdateChainInfo = 1,
    /// Update quote for a destination chain
    /// Accounts: [payer, updater, config, quote_body, system_program]
    UpdateQuote = 2,
    /// Request a quote for cross-chain execution
    /// Accounts: [config, chain_info, quote_body]
    RequestQuote = 3,
    /// Request execution quote with full details
    /// Accounts: [config, chain_info, quote_body]
    RequestExecutionQuote = 4,
}

impl TryFrom<u8> for Instruction {
    type Error = ProgramError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Instruction::Initialize),
            1 => Ok(Instruction::UpdateChainInfo),
            2 => Ok(Instruction::UpdateQuote),
            3 => Ok(Instruction::RequestQuote),
            4 => Ok(Instruction::RequestExecutionQuote),
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
        Instruction::Initialize => initialize::process(program_id, accounts, data),
        Instruction::UpdateChainInfo => update_chain_info::process(program_id, accounts, data),
        Instruction::UpdateQuote => update_quote::process(program_id, accounts, data),
        Instruction::RequestQuote => get_quote::process_request_quote(program_id, accounts, data),
        Instruction::RequestExecutionQuote => {
            get_quote::process_request_execution_quote(program_id, accounts, data)
        }
    }
}

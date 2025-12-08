//! Initialize instruction for the ExecutorQuoterRouter.
//!
//! Creates the Config PDA with the executor program ID and our chain ID.

use bytemuck::{Pod, Zeroable};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

use crate::{
    error::ExecutorQuoterRouterError,
    state::{Config, CONFIG_DISCRIMINATOR, CONFIG_SEED},
};

/// Instruction data for Initialize.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct InitializeData {
    pub executor_program_id: Pubkey,
    pub our_chain: u16,
    pub bump: u8,
    pub _padding: u8,
}

impl InitializeData {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Parse accounts
    let [payer, config_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Verify payer is signer
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse instruction data
    if data.len() < InitializeData::LEN {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    let ix_data: InitializeData =
        bytemuck::try_pod_read_unaligned(&data[..InitializeData::LEN])
            .map_err(|_| ExecutorQuoterRouterError::InvalidInstructionData)?;

    let executor_program_id = ix_data.executor_program_id;
    let our_chain = ix_data.our_chain;
    let bump = ix_data.bump;

    // Verify the config PDA
    let bump_seed = [bump];
    let expected_pda = pubkey::create_program_address(&[CONFIG_SEED, &bump_seed], program_id)
        .map_err(|_| ProgramError::InvalidSeeds)?;

    if config_account.key() != &expected_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // Create the config account
    let rent = Rent::get()?;
    let space = Config::LEN;
    let lamports = rent.minimum_balance(space);

    // Create signer seeds
    let signer_seeds = [Seed::from(CONFIG_SEED), Seed::from(&bump_seed)];
    let signers = [Signer::from(&signer_seeds[..])];

    CreateAccount {
        from: payer,
        to: config_account,
        lamports,
        space: space as u64,
        owner: program_id,
    }
    .invoke_signed(&signers)?;

    // Initialize the config data
    let mut config_data = config_account.try_borrow_mut_data()?;
    config_data[0] = CONFIG_DISCRIMINATOR;
    config_data[1] = bump;
    config_data[2..4].copy_from_slice(&our_chain.to_le_bytes());
    // Padding at bytes 4-7
    config_data[8..40].copy_from_slice(&executor_program_id);

    Ok(())
}

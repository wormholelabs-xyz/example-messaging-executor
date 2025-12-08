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
    error::ExecutorQuoterError,
    state::{Config, CONFIG_DISCRIMINATOR, CONFIG_SEED},
};

/// Instruction data for Initialize
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct InitializeData {
    pub quoter_address: Pubkey,
    pub updater_address: Pubkey,
    pub src_token_decimals: u8,
    pub bump: u8,
    pub payee_address: [u8; 32],
}

impl InitializeData {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

/// Process the Initialize instruction.
/// Creates and initializes the Config PDA.
///
/// Accounts:
/// 0. `[signer, writable]` payer - pays for account creation
/// 1. `[writable]` config - Config PDA to be created
/// 2. `[]` system_program - System program for account creation
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Parse accounts
    let [payer, config_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate payer is signer
    if !payer.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse instruction data
    if data.len() < InitializeData::LEN {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }
    let ix_data: InitializeData =
        bytemuck::try_pod_read_unaligned(&data[..InitializeData::LEN])
            .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;

    // Validate Config PDA using bump from instruction data
    let bump = ix_data.bump;
    let bump_seed = [bump];
    let derived_pda = pubkey::create_program_address(&[CONFIG_SEED, &bump_seed], program_id)
        .map_err(|_| ExecutorQuoterError::InvalidPda)?;
    if derived_pda != *config_account.key() {
        return Err(ExecutorQuoterError::InvalidPda.into());
    }

    // Ensure account is not already initialized
    if !config_account.data_is_empty() {
        return Err(ExecutorQuoterError::AlreadyInitialized.into());
    }

    // Get rent
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(Config::LEN);

    // Create signer seeds (bump_seed already defined above)
    let signer_seeds = [Seed::from(CONFIG_SEED), Seed::from(&bump_seed)];
    let signers = [Signer::from(&signer_seeds[..])];

    // Create account via CPI
    CreateAccount {
        from: payer,
        to: config_account,
        lamports,
        space: Config::LEN as u64,
        owner: program_id,
    }
    .invoke_signed(&signers)?;

    // Initialize account data
    let mut account_data = config_account.try_borrow_mut_data()?;
    let config = bytemuck::try_from_bytes_mut::<Config>(&mut account_data[..Config::LEN])
        .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;

    config.discriminator = CONFIG_DISCRIMINATOR;
    config.bump = bump;
    config.src_token_decimals = ix_data.src_token_decimals;
    config._padding = [0u8; 5];
    config.quoter_address = ix_data.quoter_address;
    config.updater_address = ix_data.updater_address;
    config.payee_address = ix_data.payee_address;

    Ok(())
}

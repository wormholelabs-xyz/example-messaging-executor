use bytemuck::{Pod, Zeroable};
use solana_program::{
    account_info::AccountInfo,
    entrypoint::ProgramResult,
    program::invoke_signed,
    program_error::ProgramError,
    pubkey::Pubkey,
    rent::Rent,
    system_instruction,
    sysvar::Sysvar,
};

use crate::{
    error::ExecutorQuoterError,
    state::{load_account, ChainInfo, Config, CHAIN_INFO_DISCRIMINATOR, CHAIN_INFO_SEED},
};

/// Instruction data for UpdateChainInfo
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct UpdateChainInfoData {
    pub chain_id: u16,
    pub enabled: u8,
    pub gas_price_decimals: u8,
    pub native_decimals: u8,
    pub bump: u8,
    pub _padding: [u8; 2],
}

impl UpdateChainInfoData {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

/// Process the UpdateChainInfo instruction.
/// Creates or updates the ChainInfo PDA for a destination chain.
///
/// Accounts:
/// 0. `[signer, writable]` payer - pays for account creation if needed
/// 1. `[signer]` updater - must match config.updater_address
/// 2. `[]` config - Config PDA for validation
/// 3. `[writable]` chain_info - ChainInfo PDA to create/update
/// 4. `[]` system_program - System program for account creation
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Parse accounts
    let [payer, updater, config_account, chain_info_account, system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate signers
    if !payer.is_signer || !updater.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse instruction data
    if data.len() < UpdateChainInfoData::LEN {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }
    let ix_data: UpdateChainInfoData =
        bytemuck::try_pod_read_unaligned(&data[..UpdateChainInfoData::LEN])
            .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;

    // Load and validate config (discriminator checked inside load_account)
    let config = load_account::<Config>(config_account, program_id)?;

    // Validate updater
    if config.updater_address != updater.key.to_bytes() {
        return Err(ExecutorQuoterError::InvalidUpdater.into());
    }

    // Prepare seeds for PDA operations
    let chain_id_bytes = ix_data.chain_id.to_le_bytes();
    let bump = ix_data.bump;
    let bump_seed = [bump];
    let seeds: &[&[u8]] = &[CHAIN_INFO_SEED, &chain_id_bytes, &bump_seed];

    // Check if account needs to be created
    let needs_creation = chain_info_account.data_is_empty();

    // If account exists, verify it's owned by this program
    if !needs_creation && chain_info_account.owner != program_id {
        return Err(ExecutorQuoterError::InvalidOwner.into());
    }

    if needs_creation {
        // Get rent
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(ChainInfo::LEN);

        // Create account via CPI
        let create_account_ix = system_instruction::create_account(
            payer.key,
            chain_info_account.key,
            lamports,
            ChainInfo::LEN as u64,
            program_id,
        );

        invoke_signed(
            &create_account_ix,
            &[payer.clone(), chain_info_account.clone(), system_program.clone()],
            &[seeds],
        )?;
    }

    // Update account data
    let mut account_data = chain_info_account.try_borrow_mut_data()?;
    let chain_info = bytemuck::try_from_bytes_mut::<ChainInfo>(&mut account_data[..ChainInfo::LEN])
        .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;

    chain_info.discriminator = CHAIN_INFO_DISCRIMINATOR;
    chain_info.enabled = ix_data.enabled;
    chain_info.chain_id = ix_data.chain_id;
    chain_info.gas_price_decimals = ix_data.gas_price_decimals;
    chain_info.native_decimals = ix_data.native_decimals;
    chain_info.bump = bump;
    chain_info._reserved = 0;

    Ok(())
}

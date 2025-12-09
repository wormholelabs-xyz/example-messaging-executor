use bytemuck::{Pod, Zeroable};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

use crate::{
    error::ExecutorQuoterError,
    state::{ChainInfo, CHAIN_INFO_DISCRIMINATOR, CHAIN_INFO_SEED},
    UPDATER_ADDRESS,
};

/// Instruction data for UpdateChainInfo.
/// Field order matches ChainInfo account bytes 2-6 for direct copy_from_slice on updates.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct UpdateChainInfoData {
    pub chain_id: u16,
    pub enabled: u8,
    pub gas_price_decimals: u8,
    pub native_decimals: u8,
    pub _padding: u8,
}

impl UpdateChainInfoData {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

/// Process the UpdateChainInfo instruction.
/// Creates or updates the ChainInfo PDA for a destination chain.
///
/// Accounts:
/// 0. `[signer, writable]` payer - pays for account creation if needed
/// 1. `[signer]` updater - must match UPDATER_ADDRESS constant
/// 2. `[writable]` chain_info - ChainInfo PDA to create/update
/// 3. `[]` system_program - System program for account creation
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Parse accounts
    let [payer, updater, chain_info_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate signers
    if !payer.is_signer() || !updater.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate instruction data length
    if data.len() < UpdateChainInfoData::LEN {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    // Validate updater against hardcoded address
    if UPDATER_ADDRESS != *updater.key() {
        return Err(ExecutorQuoterError::InvalidUpdater.into());
    }

    // Check if account needs to be created
    if chain_info_account.data_is_empty() {
        // Parse instruction data (only needed for creation)
        let ix_data: UpdateChainInfoData =
            bytemuck::try_pod_read_unaligned(&data[..UpdateChainInfoData::LEN])
                .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;

        // Derive canonical PDA and bump
        let chain_id_bytes = ix_data.chain_id.to_le_bytes();
        let (expected_pda, canonical_bump) =
            find_program_address(&[CHAIN_INFO_SEED, &chain_id_bytes], program_id);

        // Verify passed account matches expected PDA
        if *chain_info_account.key() != expected_pda {
            return Err(ExecutorQuoterError::InvalidPda.into());
        }

        // Get rent
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(ChainInfo::LEN);

        // Create signer seeds with canonical bump
        let bump_seed = [canonical_bump];
        let signer_seeds = [
            Seed::from(CHAIN_INFO_SEED),
            Seed::from(chain_id_bytes.as_slice()),
            Seed::from(&bump_seed),
        ];
        let signers = [Signer::from(&signer_seeds[..])];

        // Create account via CPI
        CreateAccount {
            from: payer,
            to: chain_info_account,
            lamports,
            space: ChainInfo::LEN as u64,
            owner: program_id,
        }
        .invoke_signed(&signers)?;

        // Initialize account data
        let chain_info = ChainInfo {
            discriminator: CHAIN_INFO_DISCRIMINATOR,
            bump: canonical_bump,
            chain_id: ix_data.chain_id,
            enabled: ix_data.enabled,
            gas_price_decimals: ix_data.gas_price_decimals,
            native_decimals: ix_data.native_decimals,
            _padding: 0,
        };
        chain_info_account
            .try_borrow_mut_data()?
            .copy_from_slice(bytemuck::bytes_of(&chain_info));
    } else {
        // Account exists - verify ownership
        if chain_info_account.owner() != program_id {
            return Err(ExecutorQuoterError::InvalidOwner.into());
        }

        // Update mutable fields directly via slice copy.
        // Layout: enabled, gas_price_decimals, native_decimals are at
        // bytes 4-7 in account data and bytes 2-5 in instruction data.
        // Safety: owner check above guarantees correct size (only our program can
        // create accounts it owns, and we always use ChainInfo::LEN).
        let mut account_data = chain_info_account.try_borrow_mut_data()?;

        // Verify discriminator (first byte)
        if account_data[0] != CHAIN_INFO_DISCRIMINATOR {
            return Err(ExecutorQuoterError::InvalidDiscriminator.into());
        }

        // Verify chain_id matches (cannot change which chain this account is for)
        if account_data[2..4] != data[0..2] {
            return Err(ExecutorQuoterError::ChainIdMismatch.into());
        }

        account_data[4..7].copy_from_slice(&data[2..5]);
    }

    Ok(())
}

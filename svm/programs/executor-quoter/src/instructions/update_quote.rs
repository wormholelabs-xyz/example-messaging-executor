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
    state::{QuoteBody, QUOTE_BODY_DISCRIMINATOR, QUOTE_SEED},
    UPDATER_ADDRESS,
};

/// Instruction data for UpdateQuote
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct UpdateQuoteData {
    pub chain_id: u16,
    pub _padding: [u8; 6],
    pub dst_price: u64,
    pub src_price: u64,
    pub dst_gas_price: u64,
    pub base_fee: u64,
}

impl UpdateQuoteData {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

/// Process the UpdateQuote instruction.
/// Creates or updates the QuoteBody PDA for a destination chain.
///
/// Accounts:
/// 0. `[signer, writable]` payer - pays for account creation if needed
/// 1. `[signer]` updater - must match UPDATER_ADDRESS constant
/// 2. `[]` _config - reserved for future use
/// 3. `[writable]` quote_body - QuoteBody PDA to create/update
/// 4. `[]` system_program - System program for account creation
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Parse accounts
    let [payer, updater, _config, quote_body_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate signers
    if !payer.is_signer() || !updater.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate instruction data length
    if data.len() < UpdateQuoteData::LEN {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    // Validate updater against hardcoded address
    if UPDATER_ADDRESS != *updater.key() {
        return Err(ExecutorQuoterError::InvalidUpdater.into());
    }

    // Check if account needs to be created
    if quote_body_account.data_is_empty() {
        // Parse instruction data (only needed for creation)
        let ix_data: UpdateQuoteData =
            bytemuck::try_pod_read_unaligned(&data[..UpdateQuoteData::LEN])
                .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;

        // Derive canonical PDA and bump
        let chain_id_bytes = ix_data.chain_id.to_le_bytes();
        let (expected_pda, canonical_bump) =
            find_program_address(&[QUOTE_SEED, &chain_id_bytes], program_id);

        // Verify passed account matches expected PDA
        if *quote_body_account.key() != expected_pda {
            return Err(ExecutorQuoterError::InvalidPda.into());
        }

        // Get rent
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(QuoteBody::LEN);

        // Create signer seeds with canonical bump
        let bump_seed = [canonical_bump];
        let signer_seeds = [
            Seed::from(QUOTE_SEED),
            Seed::from(chain_id_bytes.as_slice()),
            Seed::from(&bump_seed),
        ];
        let signers = [Signer::from(&signer_seeds[..])];

        // Create account via CPI
        CreateAccount {
            from: payer,
            to: quote_body_account,
            lamports,
            space: QuoteBody::LEN as u64,
            owner: program_id,
        }
        .invoke_signed(&signers)?;

        // Initialize account data
        let quote_body = QuoteBody {
            discriminator: QUOTE_BODY_DISCRIMINATOR,
            bump: canonical_bump,
            chain_id: ix_data.chain_id,
            _padding: [0u8; 4],
            dst_price: ix_data.dst_price,
            src_price: ix_data.src_price,
            dst_gas_price: ix_data.dst_gas_price,
            base_fee: ix_data.base_fee,
        };
        quote_body_account
            .try_borrow_mut_data()?
            .copy_from_slice(bytemuck::bytes_of(&quote_body));
    } else {
        // Account exists - verify ownership
        if quote_body_account.owner() != program_id {
            return Err(ExecutorQuoterError::InvalidOwner.into());
        }

        // Update pricing fields directly via slice copy.
        // Layout: dst_price, src_price, dst_gas_price, base_fee are at bytes 8-40
        // in both instruction data and account data.
        // Safety: owner check above guarantees correct size (only our program can
        // create accounts it owns, and we always use QuoteBody::LEN).
        let mut account_data = quote_body_account.try_borrow_mut_data()?;

        // Verify discriminator (first byte)
        if account_data[0] != QUOTE_BODY_DISCRIMINATOR {
            return Err(ExecutorQuoterError::InvalidDiscriminator.into());
        }

        // Verify chain_id matches (instruction bytes 0..2 vs account bytes 2..4)
        if data[0..2] != account_data[2..4] {
            return Err(ExecutorQuoterError::ChainIdMismatch.into());
        }

        account_data[8..40].copy_from_slice(&data[8..40]);
    }

    Ok(())
}

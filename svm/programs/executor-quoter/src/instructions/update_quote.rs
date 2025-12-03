use bytemuck::{Pod, Zeroable};
use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::Pubkey,
    sysvars::{rent::Rent, Sysvar},
    ProgramResult,
};
use pinocchio_system::instructions::CreateAccount;

use crate::{
    error::ExecutorQuoterError,
    state::{load_account, Config, QuoteBody, QUOTE_BODY_DISCRIMINATOR, QUOTE_SEED},
};

/// Instruction data for UpdateQuote
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy)]
pub struct UpdateQuoteData {
    pub chain_id: u16,
    pub bump: u8,
    pub _padding: [u8; 5],
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
/// 1. `[signer]` updater - must match config.updater_address
/// 2. `[]` config - Config PDA for validation
/// 3. `[writable]` quote_body - QuoteBody PDA to create/update
/// 4. `[]` system_program - System program for account creation
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Parse accounts
    let [payer, updater, config_account, quote_body_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Validate signers
    if !payer.is_signer() || !updater.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse instruction data
    if data.len() < UpdateQuoteData::LEN {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }
    let ix_data: UpdateQuoteData = bytemuck::try_pod_read_unaligned(&data[..UpdateQuoteData::LEN])
        .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;

    // Load and validate config (discriminator checked inside load_account)
    let config = load_account::<Config>(config_account, program_id)?;

    // Validate updater
    if config.updater_address != *updater.key() {
        return Err(ExecutorQuoterError::InvalidUpdater.into());
    }

    // Prepare seeds for PDA operations
    let chain_id_bytes = ix_data.chain_id.to_le_bytes();
    let bump = ix_data.bump;
    let bump_seed = [bump];

    // Check if account needs to be created
    let needs_creation = quote_body_account.data_is_empty();

    // If account exists, verify it's owned by this program (PDA validation happens via CPI signing during creation)
    if !needs_creation && quote_body_account.owner() != program_id {
        return Err(ExecutorQuoterError::InvalidOwner.into());
    }

    if needs_creation {
        // Get rent
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(QuoteBody::LEN);

        // Create signer seeds (bump_seed already defined above)
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
    }

    // Update account data
    let mut account_data = quote_body_account.try_borrow_mut_data()?;
    let quote_body = bytemuck::try_from_bytes_mut::<QuoteBody>(&mut account_data[..QuoteBody::LEN])
        .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;

    quote_body.discriminator = QUOTE_BODY_DISCRIMINATOR;
    quote_body._padding = [0u8; 3];
    quote_body.chain_id = ix_data.chain_id;
    quote_body.bump = bump;
    quote_body._reserved = 0;
    quote_body.dst_price = ix_data.dst_price;
    quote_body.src_price = ix_data.src_price;
    quote_body.dst_gas_price = ix_data.dst_gas_price;
    quote_body.base_fee = ix_data.base_fee;

    Ok(())
}

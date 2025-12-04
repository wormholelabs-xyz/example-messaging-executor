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
/// Accounts (ordered for zero-clone CPI):
/// 0. `[signer, writable]` payer - pays for account creation if needed
/// 1. `[writable]` quote_body - QuoteBody PDA to create/update
/// 2. `[]` system_program - System program for account creation
/// 3. `[signer]` updater - must match config.updater_address
/// 4. `[]` config - Config PDA for validation
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Validate account count
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let payer = &accounts[0];
    let quote_body_account = &accounts[1];
    let updater = &accounts[3];
    let config_account = &accounts[4];

    // Validate signers
    if !payer.is_signer || !updater.is_signer {
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
    if config.updater_address != updater.key.to_bytes() {
        return Err(ExecutorQuoterError::InvalidUpdater.into());
    }

    // Prepare seeds for PDA operations
    let chain_id_bytes = ix_data.chain_id.to_le_bytes();
    let bump = ix_data.bump;
    let bump_seed = [bump];
    let seeds: &[&[u8]] = &[QUOTE_SEED, &chain_id_bytes, &bump_seed];

    // Check if account needs to be created
    let needs_creation = quote_body_account.data_is_empty();

    // If account exists, verify it's owned by this program
    if !needs_creation && quote_body_account.owner != program_id {
        return Err(ExecutorQuoterError::InvalidOwner.into());
    }

    if needs_creation {
        // Get rent
        let rent = Rent::get()?;
        let lamports = rent.minimum_balance(QuoteBody::LEN);

        // Create account via CPI
        let create_account_ix = system_instruction::create_account(
            payer.key,
            quote_body_account.key,
            lamports,
            QuoteBody::LEN as u64,
            program_id,
        );

        // Accounts 0-2 are exactly what create_account CPI needs (payer, quote_body, system_program)
        invoke_signed(&create_account_ix, &accounts[0..3], &[seeds])?;
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

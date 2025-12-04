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
    state::{
        read_bytes32, read_u16_le, validate_account, write_u16_le, write_u64_le, write_u8,
        CONFIG_DISCRIMINATOR, CONFIG_LEN, CONFIG_UPDATER_ADDRESS_OFFSET, QUOTE_BODY_BASE_FEE_OFFSET,
        QUOTE_BODY_BUMP_OFFSET, QUOTE_BODY_CHAIN_ID_OFFSET, QUOTE_BODY_DISCRIMINATOR,
        QUOTE_BODY_DST_GAS_PRICE_OFFSET, QUOTE_BODY_DST_PRICE_OFFSET, QUOTE_BODY_LEN,
        QUOTE_BODY_SRC_PRICE_OFFSET, QUOTE_SEED,
    },
};

// UpdateQuoteData layout (40 bytes):
// Offset  Field           Type    Size
// 0-1     chain_id        u16     2
// 2       bump            u8      1
// 3-7     _padding        [u8;5]  5
// 8-15    dst_price       u64     8
// 16-23   src_price       u64     8
// 24-31   dst_gas_price   u64     8
// 32-39   base_fee        u64     8
const IX_DATA_LEN: usize = 40;
const IX_CHAIN_ID_OFFSET: usize = 0;
const IX_BUMP_OFFSET: usize = 2;
const IX_DST_PRICE_OFFSET: usize = 8;
const IX_SRC_PRICE_OFFSET: usize = 16;
const IX_DST_GAS_PRICE_OFFSET: usize = 24;
const IX_BASE_FEE_OFFSET: usize = 32;

#[inline]
fn read_u64_le(data: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
        data[offset + 4],
        data[offset + 5],
        data[offset + 6],
        data[offset + 7],
    ])
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

    // Validate instruction data length
    if data.len() < IX_DATA_LEN {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    // Validate config account
    validate_account(config_account, program_id, CONFIG_DISCRIMINATOR, CONFIG_LEN)?;

    // Read updater_address from config and validate
    {
        let config_data = config_account.try_borrow_data()?;
        let updater_address = read_bytes32(&config_data, CONFIG_UPDATER_ADDRESS_OFFSET);
        if updater_address != updater.key.as_ref() {
            return Err(ExecutorQuoterError::InvalidUpdater.into());
        }
    }

    // Read instruction data fields
    let chain_id = read_u16_le(data, IX_CHAIN_ID_OFFSET);
    let bump = data[IX_BUMP_OFFSET];

    // Prepare seeds for PDA operations
    let chain_id_bytes = chain_id.to_le_bytes();
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
        let lamports = rent.minimum_balance(QUOTE_BODY_LEN);

        // Create account via CPI
        let create_account_ix = system_instruction::create_account(
            payer.key,
            quote_body_account.key,
            lamports,
            QUOTE_BODY_LEN as u64,
            program_id,
        );

        // Accounts 0-2 are exactly what create_account CPI needs (payer, quote_body, system_program)
        invoke_signed(&create_account_ix, &accounts[0..3], &[seeds])?;
    }

    // Update account data using byte offsets
    let mut account_data = quote_body_account.try_borrow_mut_data()?;

    // Write discriminator
    write_u8(&mut account_data, 0, QUOTE_BODY_DISCRIMINATOR);

    // Write padding (bytes 1-3 are zeroed by account creation)
    write_u8(&mut account_data, 1, 0);
    write_u8(&mut account_data, 2, 0);
    write_u8(&mut account_data, 3, 0);

    // Write chain_id
    write_u16_le(&mut account_data, QUOTE_BODY_CHAIN_ID_OFFSET, chain_id);

    // Write bump
    write_u8(&mut account_data, QUOTE_BODY_BUMP_OFFSET, bump);

    // Write reserved (byte 7)
    write_u8(&mut account_data, 7, 0);

    // Write price fields
    write_u64_le(
        &mut account_data,
        QUOTE_BODY_DST_PRICE_OFFSET,
        read_u64_le(data, IX_DST_PRICE_OFFSET),
    );
    write_u64_le(
        &mut account_data,
        QUOTE_BODY_SRC_PRICE_OFFSET,
        read_u64_le(data, IX_SRC_PRICE_OFFSET),
    );
    write_u64_le(
        &mut account_data,
        QUOTE_BODY_DST_GAS_PRICE_OFFSET,
        read_u64_le(data, IX_DST_GAS_PRICE_OFFSET),
    );
    write_u64_le(
        &mut account_data,
        QUOTE_BODY_BASE_FEE_OFFSET,
        read_u64_le(data, IX_BASE_FEE_OFFSET),
    );

    Ok(())
}

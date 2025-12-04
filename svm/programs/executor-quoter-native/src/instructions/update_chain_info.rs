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
        read_bytes32, read_u16_le, validate_account, write_u16_le, write_u8,
        CHAIN_INFO_BUMP_OFFSET, CHAIN_INFO_CHAIN_ID_OFFSET, CHAIN_INFO_DISCRIMINATOR,
        CHAIN_INFO_ENABLED_OFFSET, CHAIN_INFO_GAS_PRICE_DECIMALS_OFFSET, CHAIN_INFO_LEN,
        CHAIN_INFO_NATIVE_DECIMALS_OFFSET, CHAIN_INFO_SEED, CONFIG_DISCRIMINATOR, CONFIG_LEN,
        CONFIG_UPDATER_ADDRESS_OFFSET,
    },
};

// UpdateChainInfoData layout (8 bytes):
// Offset  Field               Type    Size
// 0-1     chain_id            u16     2
// 2       enabled             u8      1
// 3       gas_price_decimals  u8      1
// 4       native_decimals     u8      1
// 5       bump                u8      1
// 6-7     _padding            [u8;2]  2
const IX_DATA_LEN: usize = 8;
const IX_CHAIN_ID_OFFSET: usize = 0;
const IX_ENABLED_OFFSET: usize = 2;
const IX_GAS_PRICE_DECIMALS_OFFSET: usize = 3;
const IX_NATIVE_DECIMALS_OFFSET: usize = 4;
const IX_BUMP_OFFSET: usize = 5;

/// Process the UpdateChainInfo instruction.
/// Creates or updates the ChainInfo PDA for a destination chain.
///
/// Accounts (ordered for zero-clone CPI):
/// 0. `[signer, writable]` payer - pays for account creation if needed
/// 1. `[writable]` chain_info - ChainInfo PDA to create/update
/// 2. `[]` system_program - System program for account creation
/// 3. `[signer]` updater - must match config.updater_address
/// 4. `[]` config - Config PDA for validation
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Validate account count
    if accounts.len() < 5 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let payer = &accounts[0];
    let chain_info_account = &accounts[1];
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
        let lamports = rent.minimum_balance(CHAIN_INFO_LEN);

        // Create account via CPI
        let create_account_ix = system_instruction::create_account(
            payer.key,
            chain_info_account.key,
            lamports,
            CHAIN_INFO_LEN as u64,
            program_id,
        );

        // Accounts 0-2 are exactly what create_account CPI needs (payer, chain_info, system_program)
        invoke_signed(&create_account_ix, &accounts[0..3], &[seeds])?;
    }

    // Update account data using byte offsets
    let mut account_data = chain_info_account.try_borrow_mut_data()?;

    // Write discriminator
    write_u8(&mut account_data, 0, CHAIN_INFO_DISCRIMINATOR);

    // Write enabled
    write_u8(&mut account_data, CHAIN_INFO_ENABLED_OFFSET, data[IX_ENABLED_OFFSET]);

    // Write chain_id
    write_u16_le(&mut account_data, CHAIN_INFO_CHAIN_ID_OFFSET, chain_id);

    // Write gas_price_decimals
    write_u8(
        &mut account_data,
        CHAIN_INFO_GAS_PRICE_DECIMALS_OFFSET,
        data[IX_GAS_PRICE_DECIMALS_OFFSET],
    );

    // Write native_decimals
    write_u8(
        &mut account_data,
        CHAIN_INFO_NATIVE_DECIMALS_OFFSET,
        data[IX_NATIVE_DECIMALS_OFFSET],
    );

    // Write bump
    write_u8(&mut account_data, CHAIN_INFO_BUMP_OFFSET, bump);

    // Write reserved (byte 7)
    write_u8(&mut account_data, 7, 0);

    Ok(())
}

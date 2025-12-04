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
        write_bytes32, write_u8, CONFIG_BUMP_OFFSET, CONFIG_DISCRIMINATOR, CONFIG_LEN,
        CONFIG_PAYEE_ADDRESS_OFFSET, CONFIG_QUOTER_ADDRESS_OFFSET, CONFIG_SEED,
        CONFIG_SRC_TOKEN_DECIMALS_OFFSET, CONFIG_UPDATER_ADDRESS_OFFSET,
    },
};

// InitializeData layout (128 bytes):
// Offset  Field               Type        Size
// 0-31    quoter_address      [u8; 32]    32
// 32-63   updater_address     [u8; 32]    32
// 64      src_token_decimals  u8          1
// 65      bump                u8          1
// 66-95   _padding            [u8; 30]    30
// 96-127  payee_address       [u8; 32]    32
const IX_DATA_LEN: usize = 128;
const IX_QUOTER_ADDRESS_OFFSET: usize = 0;
const IX_UPDATER_ADDRESS_OFFSET: usize = 32;
const IX_SRC_TOKEN_DECIMALS_OFFSET: usize = 64;
const IX_BUMP_OFFSET: usize = 65;
const IX_PAYEE_ADDRESS_OFFSET: usize = 96;

/// Process the Initialize instruction.
/// Creates and initializes the Config PDA.
///
/// Accounts:
/// 0. `[signer, writable]` payer - pays for account creation
/// 1. `[writable]` config - Config PDA to be created
/// 2. `[]` system_program - System program for account creation
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Validate account count
    if accounts.len() < 3 {
        return Err(ProgramError::NotEnoughAccountKeys);
    }

    let payer = &accounts[0];
    let config_account = &accounts[1];

    // Validate payer is signer
    if !payer.is_signer {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Validate instruction data length
    if data.len() < IX_DATA_LEN {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    // Read bump from instruction data
    let bump = data[IX_BUMP_OFFSET];
    let bump_seed = [bump];
    let seeds: &[&[u8]] = &[CONFIG_SEED, &bump_seed];

    // Validate Config PDA
    let derived_pda = Pubkey::create_program_address(seeds, program_id)
        .map_err(|_| ExecutorQuoterError::InvalidPda)?;
    if derived_pda != *config_account.key {
        return Err(ExecutorQuoterError::InvalidPda.into());
    }

    // Ensure account is not already initialized
    if !config_account.data_is_empty() {
        return Err(ExecutorQuoterError::AlreadyInitialized.into());
    }

    // Get rent
    let rent = Rent::get()?;
    let lamports = rent.minimum_balance(CONFIG_LEN);

    // Create account via CPI
    let create_account_ix = system_instruction::create_account(
        payer.key,
        config_account.key,
        lamports,
        CONFIG_LEN as u64,
        program_id,
    );

    invoke_signed(&create_account_ix, accounts, &[seeds])?;

    // Initialize account data using byte offsets
    let mut account_data = config_account.try_borrow_mut_data()?;

    // Write discriminator
    write_u8(&mut account_data, 0, CONFIG_DISCRIMINATOR);

    // Write bump
    write_u8(&mut account_data, CONFIG_BUMP_OFFSET, bump);

    // Write src_token_decimals
    write_u8(
        &mut account_data,
        CONFIG_SRC_TOKEN_DECIMALS_OFFSET,
        data[IX_SRC_TOKEN_DECIMALS_OFFSET],
    );

    // Write padding (bytes 3-7 are zeroed by account creation)

    // Write quoter_address
    let quoter_address: &[u8; 32] = data[IX_QUOTER_ADDRESS_OFFSET..IX_QUOTER_ADDRESS_OFFSET + 32]
        .try_into()
        .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;
    write_bytes32(&mut account_data, CONFIG_QUOTER_ADDRESS_OFFSET, quoter_address);

    // Write updater_address
    let updater_address: &[u8; 32] = data[IX_UPDATER_ADDRESS_OFFSET..IX_UPDATER_ADDRESS_OFFSET + 32]
        .try_into()
        .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;
    write_bytes32(
        &mut account_data,
        CONFIG_UPDATER_ADDRESS_OFFSET,
        updater_address,
    );

    // Write payee_address
    let payee_address: &[u8; 32] = data[IX_PAYEE_ADDRESS_OFFSET..IX_PAYEE_ADDRESS_OFFSET + 32]
        .try_into()
        .map_err(|_| ExecutorQuoterError::InvalidInstructionData)?;
    write_bytes32(&mut account_data, CONFIG_PAYEE_ADDRESS_OFFSET, payee_address);

    Ok(())
}

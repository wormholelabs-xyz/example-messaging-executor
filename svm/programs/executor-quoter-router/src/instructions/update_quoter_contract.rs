//! UpdateQuoterContract instruction for the ExecutorQuoterRouter.
//!
//! Registers or updates a quoter's implementation mapping using a signed governance message.
//! Uses secp256k1 ecrecover for signature verification to maintain EVM compatibility.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{self, Pubkey},
    sysvars::{clock::Clock, rent::Rent, Sysvar},
    ProgramResult,
};

#[cfg(target_os = "solana")]
use pinocchio::syscalls::{sol_keccak256, sol_secp256k1_recover};
use pinocchio_system::instructions::CreateAccount;

use crate::{
    error::ExecutorQuoterRouterError,
    state::{
        load_account, Config, QuoterRegistration, QUOTER_REGISTRATION_DISCRIMINATOR,
        QUOTER_REGISTRATION_SEED,
    },
};

use super::serialization::GovernanceMessage;

/// Secp256k1 public key length (uncompressed, without 0x04 prefix)
#[cfg(target_os = "solana")]
const SECP256K1_PUBKEY_LEN: usize = 64;

/// Keccak256 hash length
#[cfg(target_os = "solana")]
const KECCAK256_HASH_LEN: usize = 32;

/// Verifies an secp256k1 signature and recovers the Ethereum address of the signer.
///
/// This mirrors the EVM ecrecover behavior:
/// 1. Hash the message with keccak256
/// 2. Recover the public key using secp256k1_recover
/// 3. Derive the Ethereum address: keccak256(pubkey)[12:32]
///
/// Returns the 20-byte Ethereum address of the signer.
#[cfg(target_os = "solana")]
fn ecrecover(
    message: &[u8],
    signature_r: &[u8; 32],
    signature_s: &[u8; 32],
    signature_v: u8,
) -> Result<[u8; 20], ProgramError> {
    // Step 1: Compute keccak256 hash of the message
    // The syscall expects a slice format: [ptr, len] pairs
    let mut digest = [0u8; KECCAK256_HASH_LEN];

    // Build the input format for sol_keccak256: array of (ptr, len) pairs
    let message_ptr = message.as_ptr() as u64;
    let message_len = message.len() as u64;

    // Create slice descriptor: [ptr (8 bytes), len (8 bytes)]
    let slice_desc: [u64; 2] = [message_ptr, message_len];

    unsafe {
        let result = sol_keccak256(
            slice_desc.as_ptr() as *const u8,
            1, // number of slices
            digest.as_mut_ptr(),
        );
        if result != 0 {
            return Err(ExecutorQuoterRouterError::InvalidSignature.into());
        }
    }

    // Step 2: Recover public key using secp256k1_recover
    // Signature format: r (32 bytes) || s (32 bytes)
    let mut signature_rs = [0u8; 64];
    signature_rs[0..32].copy_from_slice(signature_r);
    signature_rs[32..64].copy_from_slice(signature_s);

    // Recovery ID: v - 27 (EVM uses 27/28, Solana uses 0/1)
    let recovery_id = if signature_v >= 27 {
        (signature_v - 27) as u64
    } else {
        signature_v as u64
    };

    let mut recovered_pubkey = [0u8; SECP256K1_PUBKEY_LEN];

    unsafe {
        let result = sol_secp256k1_recover(
            digest.as_ptr(),
            recovery_id,
            signature_rs.as_ptr(),
            recovered_pubkey.as_mut_ptr(),
        );
        if result != 0 {
            return Err(ExecutorQuoterRouterError::InvalidSignature.into());
        }
    }

    // Step 3: Derive Ethereum address from recovered public key
    // Ethereum address = keccak256(pubkey)[12:32]
    let mut pubkey_hash = [0u8; KECCAK256_HASH_LEN];
    let pubkey_ptr = recovered_pubkey.as_ptr() as u64;
    let pubkey_len = SECP256K1_PUBKEY_LEN as u64;
    let pubkey_slice_desc: [u64; 2] = [pubkey_ptr, pubkey_len];

    unsafe {
        let result = sol_keccak256(
            pubkey_slice_desc.as_ptr() as *const u8,
            1,
            pubkey_hash.as_mut_ptr(),
        );
        if result != 0 {
            return Err(ExecutorQuoterRouterError::InvalidSignature.into());
        }
    }

    // Take last 20 bytes as Ethereum address
    let mut eth_address = [0u8; 20];
    eth_address.copy_from_slice(&pubkey_hash[12..32]);

    Ok(eth_address)
}

/// UpdateQuoterContract instruction.
///
/// Accounts:
/// 0. `[signer, writable]` payer - Pays for account creation
/// 1. `[signer]` sender - Must match universal_sender_address in governance message
/// 2. `[]` config - Router config PDA
/// 3. `[writable]` quoter_registration - QuoterRegistration PDA (created if needed)
/// 4. `[]` system_program
///
/// Instruction data layout:
/// - bump: u8 (1 byte) - The PDA bump seed (client-derived)
/// - governance_message: [u8; 163] - The full EG01 governance message
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Minimum data: 1 byte bump + 163 byte governance message
    if data.len() < 164 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    // Parse bump from instruction data
    let bump = data[0];
    let gov_data = &data[1..];

    // Parse accounts
    let [payer, sender, config_account, quoter_registration_account, _system_program] = accounts
    else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Verify signers
    if !payer.is_signer() || !sender.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Load config to get our_chain
    let config = load_account::<Config>(config_account, program_id)?;

    // Parse governance message
    let gov_msg = GovernanceMessage::parse(gov_data)?;

    // Verify chain ID
    if gov_msg.chain_id != config.our_chain {
        return Err(ExecutorQuoterRouterError::ChainIdMismatch.into());
    }

    // Verify sender matches universal_sender_address
    // On SVM, the sender is a full 32-byte Solana pubkey.
    // Note: On EVM, this would validate that upper 12 bytes are zero (NotAnEvmAddress check).
    // On SVM, we use the full 32 bytes as Solana pubkeys, so no upper byte check is needed.
    let sender_key = sender.key();
    if gov_msg.universal_sender_address != *sender_key {
        return Err(ExecutorQuoterRouterError::InvalidSender.into());
    }

    // Verify expiry time
    let clock = Clock::get()?;
    if gov_msg.expiry_time <= clock.unix_timestamp as u64 {
        return Err(ExecutorQuoterRouterError::GovernanceExpired.into());
    }

    // Verify secp256k1 signature
    // The signed message is bytes 0-98 of the governance message (before signature)
    // This mirrors EVM: bytes32 hash = keccak256(gov[0:98]);
    #[cfg(target_os = "solana")]
    {
        let signed_message = gov_msg.signed_message(gov_data);
        let recovered_address = ecrecover(
            signed_message,
            &gov_msg.signature_r,
            &gov_msg.signature_s,
            gov_msg.signature_v,
        )?;

        // Verify the signer matches the quoter address
        // This mirrors EVM: if (signer != quoterAddr) revert InvalidSignature();
        if recovered_address != gov_msg.quoter_address {
            return Err(ExecutorQuoterRouterError::InvalidSignature.into());
        }
    }

    // Verify QuoterRegistration PDA using client-provided bump
    let bump_seed = [bump];
    let expected_pda = pubkey::create_program_address(
        &[QUOTER_REGISTRATION_SEED, &gov_msg.quoter_address[..], &bump_seed],
        program_id,
    )
    .map_err(|_| ProgramError::InvalidSeeds)?;

    if quoter_registration_account.key() != &expected_pda {
        return Err(ProgramError::InvalidSeeds);
    }

    // Extract implementation program ID from universal_contract_address
    // For SVM, this is just the full 32 bytes (a Solana pubkey)
    let implementation_program_id: Pubkey = gov_msg.universal_contract_address;

    // Create or update the quoter registration
    let bump_seed = [bump];

    if quoter_registration_account.data_is_empty() {
        // Create new account
        let rent = Rent::get()?;
        let space = QuoterRegistration::LEN;
        let lamports = rent.minimum_balance(space);

        let signer_seeds = [
            Seed::from(QUOTER_REGISTRATION_SEED),
            Seed::from(&gov_msg.quoter_address[..]),
            Seed::from(&bump_seed),
        ];
        let signers = [Signer::from(&signer_seeds[..])];

        CreateAccount {
            from: payer,
            to: quoter_registration_account,
            lamports,
            space: space as u64,
            owner: program_id,
        }
        .invoke_signed(&signers)?;
    }

    // Write registration data
    let mut reg_data = quoter_registration_account.try_borrow_mut_data()?;
    reg_data[0] = QUOTER_REGISTRATION_DISCRIMINATOR;
    reg_data[1] = bump;
    // Padding at bytes 2-3
    reg_data[4..24].copy_from_slice(&gov_msg.quoter_address);
    reg_data[24..56].copy_from_slice(&implementation_program_id);

    // TODO: Emit QuoterContractUpdate event via log

    Ok(())
}

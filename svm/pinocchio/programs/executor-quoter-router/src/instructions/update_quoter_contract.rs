//! UpdateQuoterContract instruction for the ExecutorQuoterRouter.
//!
//! Registers or updates a quoter's implementation mapping using a signed governance message.
//! Uses secp256k1 ecrecover for signature verification to maintain EVM compatibility.

use pinocchio::{
    account_info::AccountInfo,
    instruction::{Seed, Signer},
    program_error::ProgramError,
    pubkey::{find_program_address, Pubkey},
    sysvars::{clock::Clock, Sysvar},
    ProgramResult,
};

use pinocchio::syscalls::{sol_keccak256, sol_secp256k1_recover};

use crate::{
    error::ExecutorQuoterRouterError,
    state::{QuoterRegistration, QUOTER_REGISTRATION_DISCRIMINATOR, QUOTER_REGISTRATION_SEED},
    OUR_CHAIN,
};

use super::serialization::GovernanceMessage;

/// Secp256k1 public key length (uncompressed, without 0x04 prefix)
const SECP256K1_PUBKEY_LEN: usize = 64;

/// Keccak256 hash length
const KECCAK256_HASH_LEN: usize = 32;

/// Verifies a secp256k1 signature and recovers the Ethereum address of the signer.
///
/// This mirrors the shim contract ecrecover behavior (https://github.com/wormhole-foundation/wormhole/blob/main/svm/wormhole-core-shims/programs/verify-vaa/src/lib.rs):
/// 1. Hash the message with keccak256
/// 2. Recover the public key using secp256k1_recover
/// 3. Derive the Ethereum address: keccak256(pubkey)[12:32]
///
/// Returns the 20-byte Ethereum address of the signer.
fn ecrecover(
    message: &[u8],
    signature_r: &[u8; 32],
    signature_s: &[u8; 32],
    signature_v: u8,
) -> Result<[u8; 20], ProgramError> {
    // Step 1: Compute keccak256 hash of the message
    // Docs for this are here: https://github.com/solana-labs/solana/blob/master/programs/bpf_loader/src/syscalls/mod.rs
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
/// 2. `[]` _config - reserved for integrator implementations
/// 3. `[writable]` quoter_registration - QuoterRegistration PDA (created if needed)
/// 4. `[]` system_program
///
/// Instruction data layout:
/// - governance_message: [u8; 163] - The full EG01 governance message
pub fn process(program_id: &Pubkey, accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    // Minimum data: 163 byte governance message
    if data.len() < 163 {
        return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
    }

    let gov_data = data;

    // Parse accounts
    let [payer, sender, _config, quoter_registration_account, _system_program] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    // Verify signers
    if !payer.is_signer() || !sender.is_signer() {
        return Err(ProgramError::MissingRequiredSignature);
    }

    // Parse governance message
    let gov_msg = GovernanceMessage::parse(gov_data)?;

    // Verify chain ID against hardcoded constant
    if gov_msg.chain_id != OUR_CHAIN {
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

    // Extract implementation program ID from universal_contract_address
    // For SVM, this is just the full 32 bytes (a Solana pubkey)
    let implementation_program_id: Pubkey = gov_msg.universal_contract_address;

    // Check if account needs to be created
    if quoter_registration_account.data_is_empty() {
        // Derive canonical PDA and bump using find_program_address
        let (expected_pda, canonical_bump) = find_program_address(
            &[QUOTER_REGISTRATION_SEED, &gov_msg.quoter_address[..]],
            program_id,
        );

        // Verify passed account matches expected PDA
        if quoter_registration_account.key() != &expected_pda {
            return Err(ProgramError::InvalidSeeds);
        }

        // Create signer seeds with canonical bump
        let bump_seed = [canonical_bump];
        let signer_seeds = [
            Seed::from(QUOTER_REGISTRATION_SEED),
            Seed::from(&gov_msg.quoter_address[..]),
            Seed::from(&bump_seed),
        ];
        let signers = [Signer::from(&signer_seeds[..])];

        // Create account via CPI (handles pre-funded accounts to prevent griefing)
        pinocchio_system::create_account_with_minimum_balance_signed(
            quoter_registration_account,
            QuoterRegistration::LEN,
            program_id,
            payer,
            None,
            &signers,
        )?;

        // Initialize registration data
        let registration = QuoterRegistration {
            discriminator: QUOTER_REGISTRATION_DISCRIMINATOR,
            bump: canonical_bump,
            quoter_address: gov_msg.quoter_address,
            implementation_program_id,
        };
        quoter_registration_account
            .try_borrow_mut_data()?
            .copy_from_slice(bytemuck::bytes_of(&registration));
    } else {
        // Account exists - verify ownership
        if quoter_registration_account.owner() != program_id {
            return Err(ExecutorQuoterRouterError::InvalidOwner.into());
        }

        // Update registration data.
        // Safety: owner check above guarantees correct size (only our program can
        // create accounts it owns, and we always use QuoterRegistration::LEN).
        let mut reg_data = quoter_registration_account.try_borrow_mut_data()?;

        // Verify discriminator
        if reg_data[0] != QUOTER_REGISTRATION_DISCRIMINATOR {
            return Err(ExecutorQuoterRouterError::InvalidDiscriminator.into());
        }

        // Only update mutable field (implementation program ID)
        reg_data[22..54].copy_from_slice(&implementation_program_id);
    }

    Ok(())
}

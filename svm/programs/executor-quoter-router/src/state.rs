use bytemuck::{Pod, Zeroable};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::error::ExecutorQuoterRouterError;

/// Account discriminators for type safety
pub const CONFIG_DISCRIMINATOR: u8 = 1;
pub const QUOTER_REGISTRATION_DISCRIMINATOR: u8 = 2;

/// PDA seed prefixes
pub const CONFIG_SEED: &[u8] = b"config";
pub const QUOTER_REGISTRATION_SEED: &[u8] = b"quoter_registration";

/// Expiry time constant - u64::MAX means no expiry
pub const EXPIRY_TIME_MAX: u64 = u64::MAX;

/// Trait for accounts with a discriminator byte at offset 0.
pub trait Discriminator {
    const DISCRIMINATOR: u8;
}

/// Global configuration for the ExecutorQuoterRouter.
/// PDA seeds: ["config"]
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// Discriminator for account type validation
    pub discriminator: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Our chain ID (Wormhole chain ID for Solana)
    pub our_chain: u16,
    /// Padding for alignment
    pub _padding: [u8; 4],
    /// The executor program ID to CPI into for request_for_execution
    pub executor_program_id: Pubkey,
}

impl Discriminator for Config {
    const DISCRIMINATOR: u8 = CONFIG_DISCRIMINATOR;
}

impl Config {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

/// Registration mapping a quoter address to its implementation program.
/// PDA seeds: ["quoter_registration", quoter_address (20 bytes)]
///
/// This mirrors the EVM `mapping(address => IExecutorQuoter) quoterContract`.
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct QuoterRegistration {
    /// Discriminator for account type validation
    pub discriminator: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Padding for alignment
    pub _padding: [u8; 2],
    /// The quoter's Ethereum address (20 bytes) - used as the identity/key
    pub quoter_address: [u8; 20],
    /// The program ID of the quoter implementation to CPI into
    pub implementation_program_id: Pubkey,
}

impl Discriminator for QuoterRegistration {
    const DISCRIMINATOR: u8 = QUOTER_REGISTRATION_DISCRIMINATOR;
}

impl QuoterRegistration {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

/// Load a typed account from AccountInfo, validating ownership and discriminator.
/// Returns a copy of the account data.
pub fn load_account<T: Pod + Copy + Discriminator>(
    account: &AccountInfo,
    program_id: &Pubkey,
) -> Result<T, ProgramError> {
    if account.owner() != program_id {
        return Err(ExecutorQuoterRouterError::InvalidOwner.into());
    }

    let data = account.try_borrow_data()?;
    if data.len() < core::mem::size_of::<T>() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check discriminator (first byte)
    if data[0] != T::DISCRIMINATOR {
        return Err(ExecutorQuoterRouterError::InvalidDiscriminator.into());
    }

    let account = bytemuck::try_from_bytes::<T>(&data[..core::mem::size_of::<T>()])
        .map_err(|_| ProgramError::InvalidAccountData)?;
    Ok(*account)
}


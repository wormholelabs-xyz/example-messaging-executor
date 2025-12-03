use bytemuck::{Pod, Zeroable};
use pinocchio::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::error::ExecutorQuoterError;

/// Account discriminators for type safety
pub const CONFIG_DISCRIMINATOR: u8 = 1;
pub const QUOTE_BODY_DISCRIMINATOR: u8 = 2;
pub const CHAIN_INFO_DISCRIMINATOR: u8 = 3;

/// PDA seed prefixes
pub const CONFIG_SEED: &[u8] = b"config";
pub const QUOTE_SEED: &[u8] = b"quote";
pub const CHAIN_INFO_SEED: &[u8] = b"chain_info";

/// Trait for accounts with a discriminator byte at offset 0.
pub trait Discriminator {
    const DISCRIMINATOR: u8;
}

/// Global configuration for the ExecutorQuoter.
/// PDA seeds: ["config"]
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct Config {
    /// Discriminator for account type validation
    pub discriminator: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Decimals of the source chain native token (SOL = 9)
    pub src_token_decimals: u8,
    /// Padding for alignment
    pub _padding: [u8; 5],
    /// The address of the quoter (for identification purposes)
    pub quoter_address: Pubkey,
    /// The address authorized to update quotes and chain info
    pub updater_address: Pubkey,
    /// Universal address format for payee (32 bytes)
    pub payee_address: [u8; 32],
}

impl Discriminator for Config {
    const DISCRIMINATOR: u8 = CONFIG_DISCRIMINATOR;
}

impl Config {
    pub const LEN: usize = core::mem::size_of::<Self>();
}

/// On-chain quote body for a specific destination chain.
/// Mirrors the EVM OnChainQuoteBody struct.
/// PDA seeds: ["quote", chain_id (u16 le bytes)]
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct QuoteBody {
    /// Discriminator for account type validation
    pub discriminator: u8,
    /// Padding for alignment
    pub _padding: [u8; 3],
    /// The destination chain ID this quote applies to
    pub chain_id: u16,
    /// PDA bump seed
    pub bump: u8,
    /// Reserved
    pub _reserved: u8,
    /// The USD price, in 10^10, of the destination chain native currency
    pub dst_price: u64,
    /// The USD price, in 10^10, of the source chain native currency
    pub src_price: u64,
    /// The current gas price on the destination chain
    pub dst_gas_price: u64,
    /// The base fee, in source chain native currency, required by the quoter
    pub base_fee: u64,
}

impl Discriminator for QuoteBody {
    const DISCRIMINATOR: u8 = QUOTE_BODY_DISCRIMINATOR;
}

impl QuoteBody {
    pub const LEN: usize = core::mem::size_of::<Self>();

    /// Pack the quote body into a bytes32 representation (EQ01 format).
    /// Layout (32 bytes):
    /// - bytes 0-7: base_fee (u64 be)
    /// - bytes 8-15: dst_gas_price (u64 be)
    /// - bytes 16-23: src_price (u64 be)
    /// - bytes 24-31: dst_price (u64 be)
    pub fn to_bytes32(&self) -> [u8; 32] {
        let mut result = [0u8; 32];
        result[0..8].copy_from_slice(&self.base_fee.to_be_bytes());
        result[8..16].copy_from_slice(&self.dst_gas_price.to_be_bytes());
        result[16..24].copy_from_slice(&self.src_price.to_be_bytes());
        result[24..32].copy_from_slice(&self.dst_price.to_be_bytes());
        result
    }
}

/// Chain-specific configuration.
/// PDA seeds: ["chain_info", chain_id (u16 le bytes)]
#[repr(C)]
#[derive(Pod, Zeroable, Clone, Copy, Debug, PartialEq)]
pub struct ChainInfo {
    /// Discriminator for account type validation
    pub discriminator: u8,
    /// Whether this chain is enabled for quoting
    pub enabled: u8,
    /// The chain ID this info applies to
    pub chain_id: u16,
    /// Decimals used for gas price on this chain
    pub gas_price_decimals: u8,
    /// Decimals of the native token on this chain
    pub native_decimals: u8,
    /// PDA bump seed
    pub bump: u8,
    /// Reserved
    pub _reserved: u8,
}

impl Discriminator for ChainInfo {
    const DISCRIMINATOR: u8 = CHAIN_INFO_DISCRIMINATOR;
}

impl ChainInfo {
    pub const LEN: usize = core::mem::size_of::<Self>();

    pub fn is_enabled(&self) -> bool {
        self.enabled == 1
    }
}

/// Load a typed account from AccountInfo, validating ownership and discriminator.
/// Returns a copy of the account data.
pub fn load_account<T: Pod + Copy + Discriminator>(
    account: &AccountInfo,
    program_id: &Pubkey,
) -> Result<T, ProgramError> {
    if account.owner() != program_id {
        return Err(ExecutorQuoterError::InvalidOwner.into());
    }

    let data = account.try_borrow_data()?;
    if data.len() < core::mem::size_of::<T>() {
        return Err(ProgramError::InvalidAccountData);
    }

    // Check discriminator (first byte)
    if data[0] != T::DISCRIMINATOR {
        return Err(ExecutorQuoterError::InvalidDiscriminator.into());
    }

    Ok(*bytemuck::from_bytes::<T>(&data[..core::mem::size_of::<T>()]))
}

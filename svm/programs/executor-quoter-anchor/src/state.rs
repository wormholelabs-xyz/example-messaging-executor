use anchor_lang::prelude::*;

/// PDA seed prefixes
pub const CONFIG_SEED: &[u8] = b"config";
pub const QUOTE_SEED: &[u8] = b"quote";
pub const CHAIN_INFO_SEED: &[u8] = b"chain_info";

/// Global configuration for the ExecutorQuoter.
/// PDA seeds: ["config"]
#[account]
#[derive(Debug, PartialEq)]
pub struct Config {
    /// PDA bump seed
    pub bump: u8,
    /// Decimals of the source chain native token (SOL = 9)
    pub src_token_decimals: u8,
    /// The address of the quoter (for identification purposes)
    pub quoter_address: Pubkey,
    /// The address authorized to update quotes and chain info
    pub updater_address: Pubkey,
    /// Universal address format for payee (32 bytes)
    pub payee_address: [u8; 32],
}

impl Config {
    /// Account size: 8 (discriminator) + 1 + 1 + 32 + 32 + 32 = 106 bytes
    pub const LEN: usize = 8 + 1 + 1 + 32 + 32 + 32;
}

/// On-chain quote body for a specific destination chain.
/// Mirrors the EVM OnChainQuoteBody struct.
/// PDA seeds: ["quote", chain_id (u16 le bytes)]
#[account]
#[derive(Debug, PartialEq)]
pub struct QuoteBody {
    /// The destination chain ID this quote applies to
    pub chain_id: u16,
    /// PDA bump seed
    pub bump: u8,
    /// The USD price, in 10^10, of the destination chain native currency
    pub dst_price: u64,
    /// The USD price, in 10^10, of the source chain native currency
    pub src_price: u64,
    /// The current gas price on the destination chain
    pub dst_gas_price: u64,
    /// The base fee, in source chain native currency, required by the quoter
    pub base_fee: u64,
}

impl QuoteBody {
    /// Account size: 8 (discriminator) + 2 + 1 + 8 + 8 + 8 + 8 = 43 bytes
    /// Aligned to 8 bytes = 48 bytes
    pub const LEN: usize = 8 + 2 + 1 + 5 + 8 + 8 + 8 + 8; // 5 bytes padding for alignment

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
#[account]
#[derive(Debug, PartialEq)]
pub struct ChainInfo {
    /// Whether this chain is enabled for quoting
    pub enabled: bool,
    /// The chain ID this info applies to
    pub chain_id: u16,
    /// Decimals used for gas price on this chain
    pub gas_price_decimals: u8,
    /// Decimals of the native token on this chain
    pub native_decimals: u8,
    /// PDA bump seed
    pub bump: u8,
}

impl ChainInfo {
    /// Account size: 8 (discriminator) + 1 + 2 + 1 + 1 + 1 = 14 bytes
    /// Aligned to 8 bytes = 16 bytes
    pub const LEN: usize = 8 + 1 + 2 + 1 + 1 + 1 + 2; // 2 bytes padding
}

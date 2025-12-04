use solana_program::{account_info::AccountInfo, program_error::ProgramError, pubkey::Pubkey};

use crate::error::ExecutorQuoterError;

// Account discriminators
pub const CONFIG_DISCRIMINATOR: u8 = 1;
pub const QUOTE_BODY_DISCRIMINATOR: u8 = 2;
pub const CHAIN_INFO_DISCRIMINATOR: u8 = 3;

// PDA seed prefixes
pub const CONFIG_SEED: &[u8] = b"config";
pub const QUOTE_SEED: &[u8] = b"quote";
pub const CHAIN_INFO_SEED: &[u8] = b"chain_info";

// Config account layout (104 bytes)
// Offset  Field               Type        Size
// 0       discriminator       u8          1
// 1       bump                u8          1
// 2       src_token_decimals  u8          1
// 3-7     _padding            [u8; 5]     5
// 8-39    quoter_address      [u8; 32]    32
// 40-71   updater_address     [u8; 32]    32
// 72-103  payee_address       [u8; 32]    32
pub const CONFIG_LEN: usize = 104;
pub const CONFIG_BUMP_OFFSET: usize = 1;
pub const CONFIG_SRC_TOKEN_DECIMALS_OFFSET: usize = 2;
pub const CONFIG_QUOTER_ADDRESS_OFFSET: usize = 8;
pub const CONFIG_UPDATER_ADDRESS_OFFSET: usize = 40;
pub const CONFIG_PAYEE_ADDRESS_OFFSET: usize = 72;

// QuoteBody account layout (48 bytes)
// Offset  Field           Type    Size
// 0       discriminator   u8      1
// 1-3     _padding        [u8;3]  3
// 4-5     chain_id        u16     2
// 6       bump            u8      1
// 7       _reserved       u8      1
// 8-15    dst_price       u64     8
// 16-23   src_price       u64     8
// 24-31   dst_gas_price   u64     8
// 32-39   base_fee        u64     8
pub const QUOTE_BODY_LEN: usize = 48;
pub const QUOTE_BODY_CHAIN_ID_OFFSET: usize = 4;
pub const QUOTE_BODY_BUMP_OFFSET: usize = 6;
pub const QUOTE_BODY_DST_PRICE_OFFSET: usize = 8;
pub const QUOTE_BODY_SRC_PRICE_OFFSET: usize = 16;
pub const QUOTE_BODY_DST_GAS_PRICE_OFFSET: usize = 24;
pub const QUOTE_BODY_BASE_FEE_OFFSET: usize = 32;

// ChainInfo account layout (8 bytes)
// Offset  Field               Type    Size
// 0       discriminator       u8      1
// 1       enabled             u8      1
// 2-3     chain_id            u16     2
// 4       gas_price_decimals  u8      1
// 5       native_decimals     u8      1
// 6       bump                u8      1
// 7       _reserved           u8      1
pub const CHAIN_INFO_LEN: usize = 8;
pub const CHAIN_INFO_ENABLED_OFFSET: usize = 1;
pub const CHAIN_INFO_CHAIN_ID_OFFSET: usize = 2;
pub const CHAIN_INFO_GAS_PRICE_DECIMALS_OFFSET: usize = 4;
pub const CHAIN_INFO_NATIVE_DECIMALS_OFFSET: usize = 5;
pub const CHAIN_INFO_BUMP_OFFSET: usize = 6;

// Helper functions for reading data from byte slices
#[inline]
pub fn read_u8(data: &[u8], offset: usize) -> u8 {
    data[offset]
}

#[inline]
pub fn read_u16_le(data: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([data[offset], data[offset + 1]])
}

#[inline]
pub fn read_u64_le(data: &[u8], offset: usize) -> u64 {
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

#[inline]
pub fn read_bytes32<'a>(data: &'a [u8], offset: usize) -> &'a [u8] {
    &data[offset..offset + 32]
}

// Helper functions for writing data to byte slices
#[inline]
pub fn write_u8(data: &mut [u8], offset: usize, value: u8) {
    data[offset] = value;
}

#[inline]
pub fn write_u16_le(data: &mut [u8], offset: usize, value: u16) {
    let bytes = value.to_le_bytes();
    data[offset] = bytes[0];
    data[offset + 1] = bytes[1];
}

#[inline]
pub fn write_u64_le(data: &mut [u8], offset: usize, value: u64) {
    let bytes = value.to_le_bytes();
    data[offset..offset + 8].copy_from_slice(&bytes);
}

#[inline]
pub fn write_bytes32(data: &mut [u8], offset: usize, value: &[u8; 32]) {
    data[offset..offset + 32].copy_from_slice(value);
}

/// Validate account ownership and discriminator.
#[inline]
pub fn validate_account(
    account: &AccountInfo,
    program_id: &Pubkey,
    expected_discriminator: u8,
    min_len: usize,
) -> Result<(), ProgramError> {
    if account.owner != program_id {
        return Err(ExecutorQuoterError::InvalidOwner.into());
    }

    let data = account.try_borrow_data()?;
    if data.len() < min_len {
        return Err(ProgramError::InvalidAccountData);
    }

    if data[0] != expected_discriminator {
        return Err(ExecutorQuoterError::InvalidDiscriminator.into());
    }

    Ok(())
}

/// Pack quote body fields into a bytes32 representation (EQ01 format).
/// Layout (32 bytes):
/// - bytes 0-7: base_fee (u64 be)
/// - bytes 8-15: dst_gas_price (u64 be)
/// - bytes 16-23: src_price (u64 be)
/// - bytes 24-31: dst_price (u64 be)
#[inline]
pub fn pack_quote_body_to_bytes32(
    base_fee: u64,
    dst_gas_price: u64,
    src_price: u64,
    dst_price: u64,
) -> [u8; 32] {
    let mut result = [0u8; 32];
    result[0..8].copy_from_slice(&base_fee.to_be_bytes());
    result[8..16].copy_from_slice(&dst_gas_price.to_be_bytes());
    result[16..24].copy_from_slice(&src_price.to_be_bytes());
    result[24..32].copy_from_slice(&dst_price.to_be_bytes());
    result
}

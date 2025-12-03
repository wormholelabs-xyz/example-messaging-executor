//! Math utilities for quote calculation.
//!
//! This module provides U256 arithmetic and decimal normalization functions
//! needed to calculate cross-chain execution quotes.

mod u256;

pub use u256::{hi_lo, LoHi, U256};

use crate::error::ExecutorQuoterError;
use anchor_lang::prelude::*;

/// Quote decimals (prices stored with 10^10 precision)
pub const QUOTE_DECIMALS: u8 = 10;

/// Decimal resolution for intermediate calculations (10^18)
pub const DECIMAL_RESOLUTION: u8 = 18;

/// Precomputed powers of 10 for efficiency.
/// Index i contains 10^i. Supports up to 10^32 for max decimal precision.
const POW10: [u128; 33] = [
    1,
    10,
    100,
    1_000,
    10_000,
    100_000,
    1_000_000,
    10_000_000,
    100_000_000,
    1_000_000_000,
    10_000_000_000,
    100_000_000_000,
    1_000_000_000_000,
    10_000_000_000_000,
    100_000_000_000_000,
    1_000_000_000_000_000,
    10_000_000_000_000_000,
    100_000_000_000_000_000,
    1_000_000_000_000_000_000,
    10_000_000_000_000_000_000,
    100_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000_000_000,
    1_000_000_000_000_000_000_000_000_000_000,
    10_000_000_000_000_000_000_000_000_000_000,
    100_000_000_000_000_000_000_000_000_000_000,
];

/// Returns 10^exp as U256.
/// Max supported exp is 32.
#[inline]
pub fn pow10(exp: u8) -> U256 {
    debug_assert!(exp <= 32, "pow10: exp must be <= 32");
    U256::from_u128(POW10[exp as usize])
}

/// Normalize an amount from one decimal precision to another.
/// Equivalent to EVM: `normalize(amount, from, to)`
///
/// If `from > to`: divides by 10^(from-to) (truncates)
/// If `from < to`: multiplies by 10^(to-from)
/// If `from == to`: returns amount unchanged
///
/// Returns None on overflow.
#[inline]
pub fn normalize(amount: U256, from: u8, to: u8) -> Option<U256> {
    if from > to {
        let divisor = pow10(from - to);
        amount.checked_div(divisor)
    } else if from < to {
        let multiplier = pow10(to - from);
        amount.checked_mul(multiplier)
    } else {
        Some(amount)
    }
}

/// Multiply two values and divide by 10^decimals (truncates).
/// Equivalent to EVM: `mul(a, b, decimals) = (a * b) / 10^decimals`
///
/// Returns None on overflow.
#[inline]
pub fn mul_decimals(a: U256, b: U256, decimals: u8) -> Option<U256> {
    let product = a.checked_mul(b)?;
    let divisor = pow10(decimals);
    product.checked_div(divisor)
}

/// Divide a by b with decimal scaling (truncates).
/// Equivalent to EVM: `div(a, b, decimals) = (a * 10^decimals) / b`
///
/// Returns None on overflow or division by zero.
#[inline]
pub fn div_decimals(a: U256, b: U256, decimals: u8) -> Option<U256> {
    let scaled = a.checked_mul(pow10(decimals))?;
    scaled.checked_div(b)
}

/// Estimate the quote for cross-chain execution.
/// This mirrors the EVM `estimateQuote` function exactly.
///
/// # Arguments
/// * `base_fee` - Base fee in quote decimals (10^10)
/// * `src_price` - Source chain token USD price (10^10)
/// * `dst_price` - Destination chain token USD price (10^10)
/// * `dst_gas_price` - Destination chain gas price
/// * `dst_gas_price_decimals` - Decimals for dst_gas_price
/// * `dst_native_decimals` - Decimals for destination chain native token
/// * `gas_limit` - Total gas limit from relay instructions
/// * `msg_value` - Total message value from relay instructions
///
/// # Returns
/// The required payment as U256 in DECIMAL_RESOLUTION (10^18) scale.
///
/// # Errors
/// Returns `MathOverflow` on arithmetic overflow or division by zero.
pub fn estimate_quote(
    base_fee: u64,
    src_price: u64,
    dst_price: u64,
    dst_gas_price: u64,
    dst_gas_price_decimals: u8,
    dst_native_decimals: u8,
    gas_limit: u128,
    msg_value: u128,
) -> Result<U256> {
    let overflow_err = || -> Error { ExecutorQuoterError::MathOverflow.into() };

    // 1. Base fee conversion: normalize from QUOTE_DECIMALS to DECIMAL_RESOLUTION
    let src_chain_value_for_base_fee = normalize(
        U256::from_u64(base_fee),
        QUOTE_DECIMALS,
        DECIMAL_RESOLUTION,
    )
    .ok_or_else(overflow_err)?;

    // 2. Price ratio calculation
    // nSrcPrice = normalize(quote.srcPrice, QUOTE_DECIMALS, DECIMAL_RESOLUTION)
    // nDstPrice = normalize(quote.dstPrice, QUOTE_DECIMALS, DECIMAL_RESOLUTION)
    // scaledConversion = div(nDstPrice, nSrcPrice, DECIMAL_RESOLUTION)
    let n_src_price = normalize(
        U256::from_u64(src_price),
        QUOTE_DECIMALS,
        DECIMAL_RESOLUTION,
    )
    .ok_or_else(overflow_err)?;

    let n_dst_price = normalize(
        U256::from_u64(dst_price),
        QUOTE_DECIMALS,
        DECIMAL_RESOLUTION,
    )
    .ok_or_else(overflow_err)?;

    // Avoid division by zero
    if n_src_price.is_zero() {
        return Err(ExecutorQuoterError::MathOverflow.into());
    }

    let scaled_conversion =
        div_decimals(n_dst_price, n_src_price, DECIMAL_RESOLUTION).ok_or_else(overflow_err)?;

    // 3. Gas limit cost calculation
    // nGasLimitCost = normalize(gasLimit * quote.dstGasPrice, gasPriceDecimals, DECIMAL_RESOLUTION)
    // srcChainValueForGasLimit = mul(nGasLimitCost, scaledConversion, DECIMAL_RESOLUTION)
    let gas_cost = U256::from_u128(gas_limit)
        .checked_mul(U256::from_u64(dst_gas_price))
        .ok_or_else(overflow_err)?;
    let n_gas_limit_cost =
        normalize(gas_cost, dst_gas_price_decimals, DECIMAL_RESOLUTION).ok_or_else(overflow_err)?;
    let src_chain_value_for_gas_limit =
        mul_decimals(n_gas_limit_cost, scaled_conversion, DECIMAL_RESOLUTION)
            .ok_or_else(overflow_err)?;

    // 4. Message value conversion
    // nMsgValue = normalize(msgValue, nativeDecimals, DECIMAL_RESOLUTION)
    // srcChainValueForMsgValue = mul(nMsgValue, scaledConversion, DECIMAL_RESOLUTION)
    let n_msg_value = normalize(
        U256::from_u128(msg_value),
        dst_native_decimals,
        DECIMAL_RESOLUTION,
    )
    .ok_or_else(overflow_err)?;
    let src_chain_value_for_msg_value =
        mul_decimals(n_msg_value, scaled_conversion, DECIMAL_RESOLUTION).ok_or_else(overflow_err)?;

    // 5. Sum all components (all in DECIMAL_RESOLUTION scale)
    let total = src_chain_value_for_base_fee
        .checked_add(src_chain_value_for_gas_limit)
        .ok_or_else(overflow_err)?
        .checked_add(src_chain_value_for_msg_value)
        .ok_or_else(overflow_err)?;

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test estimate_quote against TypeScript/EVM reference implementation.
    /// Parameters from EVM test case (ETH -> SOL quote):
    /// - baseFee: 100
    /// - srcPrice: 2650000000 (SOL price, 10^10 decimals)
    /// - dstPrice: 160000000 (ETH price, 10^10 decimals)
    /// - dstGasPrice: 399146
    /// - gasPriceDecimals: 15
    /// - nativeDecimals: 18 (ETH)
    /// - gasLimit: 250000
    /// - msgValue: 0
    /// Expected: 6034845283018 (in DECIMAL_RESOLUTION scale, 10^18)
    #[test]
    fn test_estimate_quote_eth_to_sol() {
        let result = estimate_quote(
            100,        // base_fee
            2650000000, // src_price (ETH ~$265)
            160000000,  // dst_price (ETH ~$16 - test values)
            399146,     // dst_gas_price
            15,         // gas_price_decimals
            18,         // native_decimals (ETH)
            250000,     // gas_limit
            0,          // msg_value
        )
        .unwrap();

        assert_eq!(result, U256::from_u64(6034845283018));
    }

    #[test]
    fn test_estimate_quote_with_msg_value() {
        // Test with non-zero msg_value (1 ETH)
        let result = estimate_quote(
            100,                      // base_fee
            2650000000,               // src_price
            160000000,                // dst_price
            399146,                   // dst_gas_price
            15,                       // gas_price_decimals
            18,                       // native_decimals
            250000,                   // gas_limit
            1_000_000_000_000_000_000, // 1 ETH in wei
        )
        .unwrap();

        // Base (6034845283018) + msg_value contribution (60377358490566037)
        assert_eq!(result, U256::from_u64(60383393335849055));
    }

    #[test]
    fn test_estimate_quote_zero_gas_limit() {
        // Test with zero gas limit (only base fee)
        let result = estimate_quote(
            100,        // base_fee
            2650000000, // src_price
            160000000,  // dst_price
            399146,     // dst_gas_price
            15,         // gas_price_decimals
            18,         // native_decimals
            0,          // gas_limit = 0
            0,          // msg_value
        )
        .unwrap();

        // base_fee = 100 at QUOTE_DECIMALS (10), normalized to DECIMAL_RESOLUTION (18)
        // = 100 * 10^8 = 10000000000
        assert_eq!(result, U256::from_u64(10000000000));
    }

    #[test]
    fn test_estimate_quote_zero_src_price() {
        // Test that zero src_price returns error (division by zero protection)
        let result = estimate_quote(
            100,
            0, // zero src_price
            160000000,
            399146,
            15,
            18,
            250000,
            0,
        );

        assert!(result.is_err());
    }

    #[test]
    fn test_estimate_quote_overflow_returns_error() {
        // Test with extremely large values that would overflow
        let result = estimate_quote(
            u64::MAX,  // max base_fee
            1,         // tiny src_price (makes conversion huge)
            u64::MAX,  // max dst_price
            u64::MAX,  // max gas_price
            0,         // no decimal scaling (makes values larger)
            0,         // no decimal scaling
            u128::MAX, // max gas_limit
            u128::MAX, // max msg_value
        );

        // Should return MathOverflow error
        assert!(result.is_err());
    }

    #[test]
    fn test_u256_checked_operations_return_none_on_overflow() {
        let max = U256::new(u128::MAX, u128::MAX);
        let one = U256::from_u64(1);

        // Addition overflow returns None
        assert!(max.checked_add(one).is_none());

        // Subtraction underflow returns None
        let zero = U256::from_u64(0);
        assert!(zero.checked_sub(one).is_none());

        // Multiplication overflow returns None
        assert!(max.checked_mul(U256::from_u64(2)).is_none());

        // Division by zero returns None
        assert!(one.checked_div(zero).is_none());
    }
}

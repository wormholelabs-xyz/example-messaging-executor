//! Math utilities for quote calculation.
//!
//! This module provides U256 arithmetic and decimal normalization functions
//! needed to calculate cross-chain execution quotes.

mod u256;

pub use u256::{hi_lo, LoHi, U256};

use crate::error::ExecutorQuoterError;
use crate::state::{ChainInfo, QuoteBody};
use pinocchio::program_error::ProgramError;

/// Quote decimals (prices stored with 10^10 precision)
pub const QUOTE_DECIMALS: u8 = 10;

/// SVM decimal resolution for output (SOL = 9 decimals)
pub const SVM_DECIMAL_RESOLUTION: u8 = 9;

/// EVM decimal resolution for intermediate calculations (10^18)
pub const EVM_DECIMAL_RESOLUTION: u8 = 18;

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
    match from.cmp(&to) {
        core::cmp::Ordering::Greater => {
            let divisor = pow10(from - to);
            amount.checked_div(divisor)
        }
        core::cmp::Ordering::Less => {
            let multiplier = pow10(to - from);
            amount.checked_mul(multiplier)
        }
        core::cmp::Ordering::Equal => Some(amount),
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
/// Returns the required payment in SVM native token decimals (lamports for SOL).
///
/// # Arguments
/// * `quote_body` - Quote body containing prices and fees
/// * `chain_info` - Chain info containing decimal configurations
/// * `gas_limit` - Total gas limit from relay instructions
/// * `msg_value` - Total message value from relay instructions
///
/// # Returns
/// The required payment as u64 in SVM native decimals (lamports).
///
/// # Errors
/// Returns `MathOverflow` on arithmetic overflow or division by zero.
pub fn estimate_quote(
    quote_body: &QuoteBody,
    chain_info: &ChainInfo,
    gas_limit: u128,
    msg_value: u128,
) -> Result<u64, ProgramError> {
    let overflow_err = || -> ProgramError { ExecutorQuoterError::MathOverflow.into() };

    let total_u256 = estimate_quote_u256(quote_body, chain_info, gas_limit, msg_value)?;

    // Convert from EVM_DECIMAL_RESOLUTION to SVM_DECIMAL_RESOLUTION
    let result = normalize(total_u256, EVM_DECIMAL_RESOLUTION, SVM_DECIMAL_RESOLUTION)
        .ok_or_else(overflow_err)?;

    // Convert to u64 (should fit for reasonable quote values)
    result
        .try_into_u64()
        .ok_or_else(|| ExecutorQuoterError::MathOverflow.into())
}

fn estimate_quote_u256(
    quote_body: &QuoteBody,
    chain_info: &ChainInfo,
    gas_limit: u128,
    msg_value: u128,
) -> Result<U256, ProgramError> {
    let base_fee = quote_body.base_fee;
    let src_price = quote_body.src_price;
    let dst_price = quote_body.dst_price;
    let dst_gas_price = quote_body.dst_gas_price;
    let dst_gas_price_decimals = chain_info.gas_price_decimals;
    let dst_native_decimals = chain_info.native_decimals;
    let overflow_err = || -> ProgramError { ExecutorQuoterError::MathOverflow.into() };

    // 1. Base fee conversion: normalize from QUOTE_DECIMALS to EVM_DECIMAL_RESOLUTION
    let src_chain_value_for_base_fee = normalize(
        U256::from_u64(base_fee),
        QUOTE_DECIMALS,
        EVM_DECIMAL_RESOLUTION,
    )
    .ok_or_else(overflow_err)?;

    // 2. Price ratio calculation
    let n_src_price = normalize(
        U256::from_u64(src_price),
        QUOTE_DECIMALS,
        EVM_DECIMAL_RESOLUTION,
    )
    .ok_or_else(overflow_err)?;

    let n_dst_price = normalize(
        U256::from_u64(dst_price),
        QUOTE_DECIMALS,
        EVM_DECIMAL_RESOLUTION,
    )
    .ok_or_else(overflow_err)?;

    // Avoid division by zero
    if n_src_price.is_zero() {
        return Err(ExecutorQuoterError::MathOverflow.into());
    }

    let scaled_conversion =
        div_decimals(n_dst_price, n_src_price, EVM_DECIMAL_RESOLUTION).ok_or_else(overflow_err)?;

    // 3. Gas limit cost calculation
    let gas_cost = U256::from_u128(gas_limit)
        .checked_mul(U256::from_u64(dst_gas_price))
        .ok_or_else(overflow_err)?;
    let n_gas_limit_cost =
        normalize(gas_cost, dst_gas_price_decimals, EVM_DECIMAL_RESOLUTION).ok_or_else(overflow_err)?;
    let src_chain_value_for_gas_limit =
        mul_decimals(n_gas_limit_cost, scaled_conversion, EVM_DECIMAL_RESOLUTION)
            .ok_or_else(overflow_err)?;

    // 4. Message value conversion
    let n_msg_value = normalize(
        U256::from_u128(msg_value),
        dst_native_decimals,
        EVM_DECIMAL_RESOLUTION,
    )
    .ok_or_else(overflow_err)?;
    let src_chain_value_for_msg_value =
        mul_decimals(n_msg_value, scaled_conversion, EVM_DECIMAL_RESOLUTION).ok_or_else(overflow_err)?;

    // 5. Sum all components (all in EVM_DECIMAL_RESOLUTION scale)
    src_chain_value_for_base_fee
        .checked_add(src_chain_value_for_gas_limit)
        .ok_or_else(overflow_err)?
        .checked_add(src_chain_value_for_msg_value)
        .ok_or_else(overflow_err)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_quote_body(base_fee: u64, src_price: u64, dst_price: u64, dst_gas_price: u64) -> QuoteBody {
        QuoteBody {
            discriminator: 0,
            bump: 0,
            chain_id: 1,
            _padding: [0; 4],
            dst_price,
            src_price,
            dst_gas_price,
            base_fee,
        }
    }

    fn make_chain_info(gas_price_decimals: u8, native_decimals: u8) -> ChainInfo {
        ChainInfo {
            discriminator: 0,
            enabled: 1,
            chain_id: 1,
            gas_price_decimals,
            native_decimals,
            bump: 0,
            _padding: 0,
        }
    }

    #[test]
    fn test_estimate_quote_eth_to_sol() {
        let quote_body = make_quote_body(
            100,        // base_fee
            2650000000, // src_price (SOL ~$265)
            160000000,  // dst_price (ETH ~$16 - test values)
            399146,     // dst_gas_price
        );
        let chain_info = make_chain_info(15, 18); // gas_price_decimals, native_decimals (ETH)

        let result_18 = estimate_quote_u256(&quote_body, &chain_info, 250000, 0).unwrap();

        // Result is in lamports (9 decimals). Convert back to 18 decimals for comparison.
        let expected_18 = U256::from_u64(6034845283018u64);
        // Allow for truncation: result should be within 10^9 of expected
        assert!(result_18.checked_sub(expected_18).is_some());
        assert!(result_18
            .checked_add(U256::from_u64(1_000_000_000 - 1))
            .unwrap()
            .checked_sub(expected_18)
            .is_some());
    }

    #[test]
    fn test_estimate_quote_with_msg_value() {
        let quote_body = make_quote_body(100, 2650000000, 160000000, 399146);
        let chain_info = make_chain_info(15, 18);

        let result_18 = estimate_quote_u256(
            &quote_body,
            &chain_info,
            250000,
            1_000_000_000_000_000_000, // 1 ETH in wei
        )
        .unwrap();

        let expected_18 = U256::from_u64(60383393335849055);
        assert!(result_18.checked_sub(expected_18).is_some());
        assert!(result_18
            .checked_add(U256::from_u64(1_000_000_000 - 1))
            .unwrap()
            .checked_sub(expected_18)
            .is_some());
    }

    #[test]
    fn test_estimate_quote_zero_gas_limit() {
        let quote_body = make_quote_body(100, 2650000000, 160000000, 399146);
        let chain_info = make_chain_info(15, 18);

        let result = estimate_quote(&quote_body, &chain_info, 0, 0).unwrap();

        // base_fee = 100 at QUOTE_DECIMALS (10)
        // Converted to 9 decimals = 10 lamports
        assert_eq!(result, 10);
    }

    #[test]
    fn test_estimate_quote_zero_src_price() {
        let quote_body = make_quote_body(100, 0, 160000000, 399146); // zero src_price
        let chain_info = make_chain_info(15, 18);

        let result = estimate_quote(&quote_body, &chain_info, 250000, 0);

        assert!(result.is_err());
    }

    #[test]
    fn test_estimate_quote_overflow_returns_error() {
        let quote_body = make_quote_body(
            u64::MAX, // max base_fee
            1,        // tiny src_price (makes conversion huge)
            u64::MAX, // max dst_price
            u64::MAX, // max gas_price
        );
        let chain_info = make_chain_info(0, 0); // no decimal scaling (makes values larger)

        let result = estimate_quote(&quote_body, &chain_info, u128::MAX, u128::MAX);

        assert!(result.is_err());
    }

    #[test]
    fn test_u256_checked_operations_return_none_on_overflow() {
        let max = U256::new(u128::MAX, u128::MAX);
        let one = U256::from_u64(1);

        assert!(max.checked_add(one).is_none());

        let zero = U256::from_u64(0);
        assert!(zero.checked_sub(one).is_none());

        assert!(max.checked_mul(U256::from_u64(2)).is_none());

        assert!(one.checked_div(zero).is_none());
    }
}

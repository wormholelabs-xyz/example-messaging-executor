// cSpell:ignore Muldiv qhat rhat

//! 256-bit unsigned integer implementation.
//!
//! Ported from Orca Whirlpool's U256Muldiv implementation.
//! Original: https://github.com/orca-so/whirlpools/blob/main/programs/whirlpool/src/math/u256_math.rs

use core::cmp::Ordering;

const NUM_WORDS: usize = 4;
const U64_MAX: u128 = u64::MAX as u128;
const U64_RESOLUTION: u32 = 64;

/// A 256-bit unsigned integer represented as 4 x 64-bit words.
/// Words are stored in little-endian order (items[0] is least significant).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct U256 {
    pub items: [u64; NUM_WORDS],
}

impl U256 {
    /// Creates a new U256 from two 128-bit halves.
    /// `h` is the high 128 bits, `l` is the low 128 bits.
    pub fn new(h: u128, l: u128) -> Self {
        U256 {
            items: [l.lo(), l.hi(), h.lo(), h.hi()],
        }
    }

    /// Creates a U256 from a u64 value.
    #[inline]
    pub fn from_u64(v: u64) -> Self {
        Self::new(0, v as u128)
    }

    /// Creates a U256 from a u128 value.
    #[inline]
    pub fn from_u128(v: u128) -> Self {
        Self::new(0, v)
    }

    fn copy(&self) -> Self {
        let mut items: [u64; NUM_WORDS] = [0; NUM_WORDS];
        items.copy_from_slice(&self.items);
        U256 { items }
    }

    fn update_word(&mut self, index: usize, value: u64) {
        self.items[index] = value;
    }

    fn num_words(&self) -> usize {
        for i in (0..self.items.len()).rev() {
            if self.items[i] != 0 {
                return i + 1;
            }
        }
        0
    }

    /// Gets the word at the given index.
    pub fn get_word(&self, index: usize) -> u64 {
        self.items[index]
    }

    /// Gets the word at the given index as u128.
    pub fn get_word_u128(&self, index: usize) -> u128 {
        self.items[index] as u128
    }

    /// Converts to big-endian byte array (32 bytes).
    /// Most significant byte first, matching EVM uint256 representation.
    pub fn to_be_bytes(&self) -> [u8; 32] {
        let mut result = [0u8; 32];
        // items[3] is most significant, items[0] is least significant
        result[0..8].copy_from_slice(&self.items[3].to_be_bytes());
        result[8..16].copy_from_slice(&self.items[2].to_be_bytes());
        result[16..24].copy_from_slice(&self.items[1].to_be_bytes());
        result[24..32].copy_from_slice(&self.items[0].to_be_bytes());
        result
    }

    /// Logical-left shift by one word (64 bits).
    pub fn shift_word_left(&self) -> Self {
        let mut result = U256::new(0, 0);
        for i in (0..NUM_WORDS - 1).rev() {
            result.items[i + 1] = self.items[i];
        }
        result
    }

    /// Logical-left shift by arbitrary amount.
    pub fn shift_left(&self, mut shift_amount: u32) -> Self {
        if shift_amount >= U64_RESOLUTION * (NUM_WORDS as u32) {
            return U256::new(0, 0);
        }

        let mut result = self.copy();

        while shift_amount >= U64_RESOLUTION {
            result = result.shift_word_left();
            shift_amount -= U64_RESOLUTION;
        }

        if shift_amount == 0 {
            return result;
        }

        for i in (1..NUM_WORDS).rev() {
            result.items[i] = (result.items[i] << shift_amount)
                | (result.items[i - 1] >> (U64_RESOLUTION - shift_amount));
        }

        result.items[0] <<= shift_amount;
        result
    }

    /// Logical-right shift by one word (64 bits).
    pub fn shift_word_right(&self) -> Self {
        let mut result = U256::new(0, 0);
        for i in 0..NUM_WORDS - 1 {
            result.items[i] = self.items[i + 1]
        }
        result
    }

    /// Logical-right shift by arbitrary amount.
    pub fn shift_right(&self, mut shift_amount: u32) -> Self {
        if shift_amount >= U64_RESOLUTION * (NUM_WORDS as u32) {
            return U256::new(0, 0);
        }

        let mut result = self.copy();

        while shift_amount >= U64_RESOLUTION {
            result = result.shift_word_right();
            shift_amount -= U64_RESOLUTION;
        }

        if shift_amount == 0 {
            return result;
        }

        for i in 0..NUM_WORDS - 1 {
            result.items[i] = (result.items[i] >> shift_amount)
                | (result.items[i + 1] << (U64_RESOLUTION - shift_amount));
        }

        result.items[3] >>= shift_amount;
        result
    }

    /// Equality comparison.
    #[allow(clippy::should_implement_trait)]
    pub fn eq(&self, other: U256) -> bool {
        for i in 0..self.items.len() {
            if self.items[i] != other.items[i] {
                return false;
            }
        }
        true
    }

    /// Less than comparison.
    pub fn lt(&self, other: U256) -> bool {
        for i in (0..self.items.len()).rev() {
            match self.items[i].cmp(&other.items[i]) {
                Ordering::Less => return true,
                Ordering::Greater => return false,
                Ordering::Equal => {}
            }
        }
        false
    }

    /// Greater than comparison.
    pub fn gt(&self, other: U256) -> bool {
        for i in (0..self.items.len()).rev() {
            match self.items[i].cmp(&other.items[i]) {
                Ordering::Less => return false,
                Ordering::Greater => return true,
                Ordering::Equal => {}
            }
        }
        false
    }

    /// Less than or equal comparison.
    pub fn lte(&self, other: U256) -> bool {
        for i in (0..self.items.len()).rev() {
            match self.items[i].cmp(&other.items[i]) {
                Ordering::Less => return true,
                Ordering::Greater => return false,
                Ordering::Equal => {}
            }
        }
        true
    }

    /// Greater than or equal comparison.
    pub fn gte(&self, other: U256) -> bool {
        for i in (0..self.items.len()).rev() {
            match self.items[i].cmp(&other.items[i]) {
                Ordering::Less => return false,
                Ordering::Greater => return true,
                Ordering::Equal => {}
            }
        }
        true
    }

    /// Try to convert to u128. Returns None if value exceeds u128::MAX.
    pub fn try_into_u128(&self) -> Option<u128> {
        if self.num_words() > 2 {
            return None;
        }
        Some(((self.items[1] as u128) << U64_RESOLUTION) | (self.items[0] as u128))
    }

    /// Try to convert to u64. Returns None if value exceeds u64::MAX.
    pub fn try_into_u64(&self) -> Option<u64> {
        if self.num_words() > 1 {
            return None;
        }
        Some(self.items[0])
    }

    /// Returns true if this value is zero.
    pub fn is_zero(self) -> bool {
        for i in 0..NUM_WORDS {
            if self.items[i] != 0 {
                return false;
            }
        }
        true
    }

    /// Checked addition. Returns None on overflow.
    pub fn checked_add(&self, other: U256) -> Option<Self> {
        let mut result = U256::new(0, 0);

        let mut carry = 0u128;
        for i in 0..NUM_WORDS {
            let x = self.get_word_u128(i);
            let y = other.get_word_u128(i);
            let t = x + y + carry;
            result.update_word(i, t.lo());
            carry = t.hi_u128();
        }

        // If there's remaining carry, we overflowed
        if carry != 0 {
            return None;
        }

        Some(result)
    }

    /// Checked subtraction. Returns None on underflow.
    pub fn checked_sub(&self, other: U256) -> Option<Self> {
        // Check if self < other (would underflow)
        if self.lt(other) {
            return None;
        }

        let mut result = U256::new(0, 0);

        let mut carry = 0u64;
        for i in 0..NUM_WORDS {
            let x = self.get_word(i);
            let y = other.get_word(i);
            let (t0, overflowing0) = x.overflowing_sub(y);
            let (t1, overflowing1) = t0.overflowing_sub(carry);
            result.update_word(i, t1);
            carry = if overflowing0 || overflowing1 { 1 } else { 0 };
        }

        Some(result)
    }

    /// Checked multiplication. Returns None on overflow.
    pub fn checked_mul(&self, other: U256) -> Option<Self> {
        let mut result = U256::new(0, 0);

        let m = self.num_words();
        let n = other.num_words();

        // Quick overflow check: if sum of word counts > NUM_WORDS, likely overflow
        // (not guaranteed, but catches obvious cases early)
        if m + n > NUM_WORDS + 1 {
            return None;
        }

        for j in 0..n {
            let mut k = 0u128;
            for i in 0..m {
                let x = self.get_word_u128(i);
                let y = other.get_word_u128(j);
                if i + j < NUM_WORDS {
                    let z = result.get_word_u128(i + j);
                    let t = x * y + z + k;
                    result.update_word(i + j, t.lo());
                    k = t.hi_u128();
                } else if x * y != 0 {
                    // Would write beyond NUM_WORDS with non-zero value
                    return None;
                }
            }

            if j + m < NUM_WORDS {
                result.update_word(j + m, k as u64);
            } else if k != 0 {
                // Carry would overflow
                return None;
            }
        }

        Some(result)
    }

    /// Checked division (truncates toward zero, like Solidity).
    /// Returns None on division by zero.
    pub fn checked_div(&self, mut divisor: U256) -> Option<Self> {
        let mut dividend = self.copy();
        let mut quotient = U256::new(0, 0);

        let num_dividend_words = dividend.num_words();
        let num_divisor_words = divisor.num_words();

        if num_divisor_words == 0 {
            // Division by zero
            return None;
        }

        // Case 0: Dividend is 0
        if num_dividend_words == 0 {
            return Some(U256::new(0, 0));
        }

        // Case 1: Dividend < divisor
        if num_dividend_words < num_divisor_words {
            return Some(U256::new(0, 0));
        }

        // Case 2: Both fit in u128
        if num_dividend_words < 3 {
            let dividend_u128 = dividend.try_into_u128().unwrap();
            let divisor_u128 = divisor.try_into_u128().unwrap();
            let quotient_u128 = dividend_u128 / divisor_u128;
            return Some(U256::new(0, quotient_u128));
        }

        // Case 3: Single-word divisor
        if num_divisor_words == 1 {
            let mut k = 0u128;
            for j in (0..num_dividend_words).rev() {
                let d1 = hi_lo(k.lo(), dividend.get_word(j));
                let d2 = divisor.get_word_u128(0);
                let q = d1 / d2;
                k = d1 - d2 * q;
                quotient.update_word(j, q.lo());
            }
            return Some(quotient);
        }

        // Normalize the division by shifting left
        let s = divisor.get_word(num_divisor_words - 1).leading_zeros();
        let b = dividend.get_word(num_dividend_words - 1).leading_zeros();

        let mut dividend_carry_space: u64 = 0;
        if num_dividend_words == NUM_WORDS && b < s {
            dividend_carry_space = dividend.items[num_dividend_words - 1] >> (U64_RESOLUTION - s);
        }
        dividend = dividend.shift_left(s);
        divisor = divisor.shift_left(s);

        for j in (0..num_dividend_words - num_divisor_words + 1).rev() {
            let result = div_loop(
                j,
                num_divisor_words,
                dividend,
                &mut dividend_carry_space,
                divisor,
                quotient,
            );
            quotient = result.0;
            dividend = result.1;
        }

        Some(quotient)
    }
}

/// Trait for extracting high/low parts of u128.
pub trait LoHi {
    fn lo(self) -> u64;
    fn hi(self) -> u64;
    fn lo_u128(self) -> u128;
    fn hi_u128(self) -> u128;
}

impl LoHi for u128 {
    #[inline]
    fn lo(self) -> u64 {
        (self & U64_MAX) as u64
    }

    #[inline]
    fn lo_u128(self) -> u128 {
        self & U64_MAX
    }

    #[inline]
    fn hi(self) -> u64 {
        (self >> U64_RESOLUTION) as u64
    }

    #[inline]
    fn hi_u128(self) -> u128 {
        self >> U64_RESOLUTION
    }
}

/// Combines high and low u64 into u128.
#[inline]
pub fn hi_lo(hi: u64, lo: u64) -> u128 {
    ((hi as u128) << U64_RESOLUTION) | (lo as u128)
}

/// Helper function for the division algorithm.
fn div_loop(
    index: usize,
    num_divisor_words: usize,
    mut dividend: U256,
    dividend_carry_space: &mut u64,
    divisor: U256,
    mut quotient: U256,
) -> (U256, U256) {
    let use_carry = (index + num_divisor_words) == NUM_WORDS;
    let div_hi = if use_carry {
        *dividend_carry_space
    } else {
        dividend.get_word(index + num_divisor_words)
    };
    let d0 = hi_lo(div_hi, dividend.get_word(index + num_divisor_words - 1));
    let d1 = divisor.get_word_u128(num_divisor_words - 1);

    let mut qhat = d0 / d1;
    let mut rhat = d0 - d1 * qhat;

    let d0_2 = dividend.get_word(index + num_divisor_words - 2);
    let d1_2 = divisor.get_word_u128(num_divisor_words - 2);

    let mut cmp1 = hi_lo(rhat.lo(), d0_2);
    let mut cmp2 = qhat.wrapping_mul(d1_2);

    while qhat.hi() != 0 || cmp2 > cmp1 {
        qhat -= 1;
        rhat += d1;
        if rhat.hi() != 0 {
            break;
        }

        cmp1 = hi_lo(rhat.lo(), cmp1.lo());
        cmp2 -= d1_2;
    }

    let mut k = 0;
    let mut t;
    for i in 0..num_divisor_words {
        let p = qhat * (divisor.get_word_u128(i));
        t = (dividend.get_word_u128(index + i))
            .wrapping_sub(k)
            .wrapping_sub(p.lo_u128());
        dividend.update_word(index + i, t.lo());
        k = ((p >> U64_RESOLUTION) as u64).wrapping_sub((t >> U64_RESOLUTION) as u64) as u128;
    }

    let d_head = if use_carry {
        *dividend_carry_space as u128
    } else {
        dividend.get_word_u128(index + num_divisor_words)
    };

    t = d_head.wrapping_sub(k);
    if use_carry {
        *dividend_carry_space = t.lo();
    } else {
        dividend.update_word(index + num_divisor_words, t.lo());
    }

    if k > d_head {
        qhat -= 1;
        k = 0;
        for i in 0..num_divisor_words {
            t = dividend
                .get_word_u128(index + i)
                .wrapping_add(divisor.get_word_u128(i))
                .wrapping_add(k);
            dividend.update_word(index + i, t.lo());
            k = t >> U64_RESOLUTION;
        }

        let new_carry = dividend
            .get_word_u128(index + num_divisor_words)
            .wrapping_add(k)
            .lo();
        if use_carry {
            *dividend_carry_space = new_carry
        } else {
            dividend.update_word(
                index + num_divisor_words,
                dividend
                    .get_word_u128(index + num_divisor_words)
                    .wrapping_add(k)
                    .lo(),
            );
        }
    }

    quotient.update_word(index, qhat.lo());

    (quotient, dividend)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
        let a = U256::from_u128(100);
        let b = U256::from_u128(50);

        // Addition
        let sum = a.checked_add(b).unwrap();
        assert_eq!(sum.try_into_u128(), Some(150));

        // Subtraction
        let diff = a.checked_sub(b).unwrap();
        assert_eq!(diff.try_into_u128(), Some(50));

        // Multiplication
        let prod = a.checked_mul(b).unwrap();
        assert_eq!(prod.try_into_u128(), Some(5000));

        // Division
        let quot = a.checked_div(b).unwrap();
        assert_eq!(quot.try_into_u128(), Some(2));
    }

    #[test]
    fn test_large_multiplication() {
        let a = U256::from_u128(u128::MAX);
        let b = U256::from_u128(2);
        let prod = a.checked_mul(b).unwrap();

        assert!(prod.try_into_u128().is_none());
        assert!(prod.gt(U256::from_u128(u128::MAX)));
    }

    #[test]
    fn test_division_truncates() {
        let a = U256::from_u128(100);
        let b = U256::from_u128(30);

        let quot = a.checked_div(b).unwrap();
        assert_eq!(quot.try_into_u128(), Some(3));
    }

    #[test]
    fn test_division_by_zero() {
        let a = U256::from_u128(100);
        let b = U256::from_u128(0);

        assert!(a.checked_div(b).is_none());
    }

    #[test]
    fn test_overflow_errors() {
        let max = U256::new(u128::MAX, u128::MAX);
        let one = U256::from_u64(1);

        assert!(max.checked_add(one).is_none());

        let zero = U256::from_u64(0);
        assert!(zero.checked_sub(one).is_none());

        assert!(max.checked_mul(U256::from_u64(2)).is_none());
    }

    #[test]
    fn test_pow10_values() {
        use super::super::pow10;

        assert_eq!(pow10(0).try_into_u64(), Some(1));
        assert_eq!(pow10(1).try_into_u64(), Some(10));
        assert_eq!(pow10(9).try_into_u64(), Some(1_000_000_000));
        assert_eq!(pow10(18).try_into_u64(), Some(1_000_000_000_000_000_000));
        assert_eq!(
            pow10(32).try_into_u128(),
            Some(100_000_000_000_000_000_000_000_000_000_000)
        );
    }
}

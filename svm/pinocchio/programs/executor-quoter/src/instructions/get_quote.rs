use executor_requests::parse_relay_instructions;
use pinocchio::{
    account_info::AccountInfo, cpi::set_return_data, program_error::ProgramError, pubkey::Pubkey,
    ProgramResult,
};

use crate::{
    error::{relay_parse_error_to_program_error, ExecutorQuoterError},
    math,
    state::{load_account, ChainInfo, QuoteBody},
    PAYEE_ADDRESS,
};

/// Minimum instruction data length: dst_chain (2) + dst_addr (32) + refund_addr (32) + request_bytes_len (4) = 70.
const MIN_DATA_LEN: usize = 70;

/// Parse relay instructions from the common quote instruction data layout and compute the quote.
///
/// Instruction data layout (after 8-byte discriminator, stripped by entrypoint):
/// - dst_chain: u16 (offset 0)
/// - dst_addr: [u8; 32] (offset 2)
/// - refund_addr: [u8; 32] (offset 34)
/// - request_bytes_len: u32 (offset 66)
/// - request_bytes: [u8; request_bytes_len] (offset 70)
/// - relay_instructions_len: u32 (offset 70 + request_bytes_len)
/// - relay_instructions: [u8; relay_instructions_len]
#[inline(always)]
fn compute_quote(
    quote_body: &QuoteBody,
    chain_info: &ChainInfo,
    data: &[u8],
) -> Result<u64, ProgramError> {
    if data.len() < MIN_DATA_LEN {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    // Read request_bytes_len to skip past request_bytes
    let mut len_bytes = [0u8; 4];
    len_bytes.copy_from_slice(&data[66..70]);
    let request_bytes_len = u32::from_le_bytes(len_bytes) as usize;
    let relay_start = 70 + request_bytes_len;

    if data.len() < relay_start + 4 {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    let mut relay_len_bytes = [0u8; 4];
    relay_len_bytes.copy_from_slice(&data[relay_start..relay_start + 4]);
    let relay_instructions_len = u32::from_le_bytes(relay_len_bytes) as usize;

    let relay_data_start = relay_start + 4;
    if data.len() < relay_data_start + relay_instructions_len {
        return Err(ExecutorQuoterError::InvalidInstructionData.into());
    }

    let relay_instructions = &data[relay_data_start..relay_data_start + relay_instructions_len];

    let (gas_limit, msg_value) =
        parse_relay_instructions(relay_instructions).map_err(relay_parse_error_to_program_error)?;

    math::estimate_quote(quote_body, chain_info, gas_limit, msg_value)
}

/// Load and validate the chain_info and quote_body accounts shared by both quote instructions.
#[inline(always)]
fn load_quote_accounts(
    chain_info_account: &AccountInfo,
    quote_body_account: &AccountInfo,
    program_id: &Pubkey,
) -> Result<(ChainInfo, QuoteBody), ProgramError> {
    let chain_info = load_account::<ChainInfo>(chain_info_account, program_id)?;
    if !chain_info.is_enabled() {
        return Err(ExecutorQuoterError::ChainDisabled.into());
    }
    let quote_body = load_account::<QuoteBody>(quote_body_account, program_id)?;
    Ok((chain_info, quote_body))
}

/// Process RequestQuote instruction.
/// Returns the required payment amount for cross-chain execution.
///
/// Accounts:
/// 0. `[]` config - reserved for interface compatibility
/// 1. `[]` chain_info - ChainInfo PDA for destination chain
/// 2. `[]` quote_body - QuoteBody PDA for destination chain
pub fn process_request_quote(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    let [_config, chain_info_account, quote_body_account] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let (chain_info, quote_body) =
        load_quote_accounts(chain_info_account, quote_body_account, program_id)?;

    let required_payment = compute_quote(&quote_body, &chain_info, data)?;

    set_return_data(&required_payment.to_be_bytes());

    Ok(())
}

/// Process RequestExecutionQuote instruction.
/// Returns the required payment, payee address, and quote body.
///
/// Accounts:
/// 0. `[]` config - reserved for interface compatibility
/// 1. `[]` chain_info - ChainInfo PDA for destination chain
/// 2. `[]` quote_body - QuoteBody PDA for destination chain
/// 3. `[]` event_cpi - reserved for interface compatibility
pub fn process_request_execution_quote(
    program_id: &Pubkey,
    accounts: &[AccountInfo],
    data: &[u8],
) -> ProgramResult {
    // _config and event_cpi are required but unused in this implementation.
    // Future quoter implementations may use them.
    let [_config, chain_info_account, quote_body_account, _event_cpi] = accounts else {
        return Err(ProgramError::NotEnoughAccountKeys);
    };

    let (chain_info, quote_body) =
        load_quote_accounts(chain_info_account, quote_body_account, program_id)?;

    let required_payment = compute_quote(&quote_body, &chain_info, data)?;

    // Return data layout (72 bytes, all big-endian):
    // - bytes 0-7: required_payment (u64)
    // - bytes 8-39: payee_address (32 bytes)
    // - bytes 40-71: quote_body (32 bytes, EQ01 format)
    let mut return_data = [0u8; 72];
    return_data[0..8].copy_from_slice(&required_payment.to_be_bytes());
    return_data[8..40].copy_from_slice(&PAYEE_ADDRESS);
    return_data[40..72].copy_from_slice(&quote_body.to_bytes32());

    set_return_data(&return_data);

    Ok(())
}

#[cfg(test)]
mod tests {
    extern crate alloc;
    use alloc::vec::Vec;

    use super::*;

    fn make_test_quote_body() -> QuoteBody {
        QuoteBody {
            discriminator: 1,
            bump: 0,
            chain_id: 2,
            _padding: [0; 4],
            dst_price: 160_000_000,
            src_price: 2_650_000_000,
            dst_gas_price: 399_146,
            base_fee: 100,
        }
    }

    fn make_test_chain_info() -> ChainInfo {
        ChainInfo {
            discriminator: 2,
            bump: 0,
            chain_id: 2,
            enabled: 1,
            gas_price_decimals: 15,
            native_decimals: 18,
            _padding: 0,
        }
    }

    /// Build instruction data in the post-discriminator layout that compute_quote expects.
    fn build_quote_data(gas_limit: u128, msg_value: u128) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&2u16.to_le_bytes()); // dst_chain
        data.extend_from_slice(&[0xab; 32]); // dst_addr
        data.extend_from_slice(&[0xcd; 32]); // refund_addr
        data.extend_from_slice(&0u32.to_le_bytes()); // request_bytes_len = 0
                                                     // relay_instructions: 1 byte type + 16 gas_limit + 16 msg_value = 33
        data.extend_from_slice(&33u32.to_le_bytes()); // relay_instructions_len
        data.push(1); // RELAY_IX_GAS
        data.extend_from_slice(&gas_limit.to_be_bytes());
        data.extend_from_slice(&msg_value.to_be_bytes());
        data
    }

    /// Verify compute_quote matches a direct call to math::estimate_quote for the same inputs.
    #[test]
    fn test_compute_quote_equivalence_250k_gas() {
        let qb = make_test_quote_body();
        let ci = make_test_chain_info();
        let data = build_quote_data(250_000, 0);

        let via_compute = compute_quote(&qb, &ci, &data).unwrap();
        let via_math = math::estimate_quote(&qb, &ci, 250_000, 0).unwrap();
        assert_eq!(via_compute, via_math);
    }

    #[test]
    fn test_compute_quote_equivalence_500k_gas_1eth() {
        let qb = make_test_quote_body();
        let ci = make_test_chain_info();
        let one_eth = 1_000_000_000_000_000_000u128;
        let data = build_quote_data(500_000, one_eth);

        let via_compute = compute_quote(&qb, &ci, &data).unwrap();
        let via_math = math::estimate_quote(&qb, &ci, 500_000, one_eth).unwrap();
        assert_eq!(via_compute, via_math);
    }

    #[test]
    fn test_compute_quote_equivalence_zero_gas() {
        let qb = make_test_quote_body();
        let ci = make_test_chain_info();
        let data = build_quote_data(0, 0);

        let via_compute = compute_quote(&qb, &ci, &data).unwrap();
        let via_math = math::estimate_quote(&qb, &ci, 0, 0).unwrap();
        assert_eq!(via_compute, via_math);
    }

    #[test]
    fn test_compute_quote_with_request_bytes() {
        let qb = make_test_quote_body();
        let ci = make_test_chain_info();
        // Build data with non-zero request_bytes to test skip logic
        let mut data = Vec::new();
        data.extend_from_slice(&2u16.to_le_bytes()); // dst_chain
        data.extend_from_slice(&[0xab; 32]); // dst_addr
        data.extend_from_slice(&[0xcd; 32]); // refund_addr
        let request_bytes = [0xFFu8; 50]; // 50 bytes of request data
        data.extend_from_slice(&50u32.to_le_bytes()); // request_bytes_len = 50
        data.extend_from_slice(&request_bytes);
        data.extend_from_slice(&33u32.to_le_bytes()); // relay_instructions_len
        data.push(1); // RELAY_IX_GAS
        data.extend_from_slice(&250_000u128.to_be_bytes());
        data.extend_from_slice(&0u128.to_be_bytes());

        let via_compute = compute_quote(&qb, &ci, &data).unwrap();
        let via_math = math::estimate_quote(&qb, &ci, 250_000, 0).unwrap();
        assert_eq!(via_compute, via_math);
    }

    #[test]
    fn test_compute_quote_rejects_short_data() {
        let qb = make_test_quote_body();
        let ci = make_test_chain_info();
        let data = [0u8; 69]; // 1 byte short of MIN_DATA_LEN
        assert!(compute_quote(&qb, &ci, &data).is_err());
    }

    #[test]
    fn test_compute_quote_rejects_truncated_relay() {
        let qb = make_test_quote_body();
        let ci = make_test_chain_info();
        let mut data = Vec::new();
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&[0xab; 32]);
        data.extend_from_slice(&[0xcd; 32]);
        data.extend_from_slice(&0u32.to_le_bytes()); // request_bytes_len = 0
        data.extend_from_slice(&100u32.to_le_bytes()); // claims 100 bytes of relay data
                                                       // but provides none
        assert!(compute_quote(&qb, &ci, &data).is_err());
    }

    #[test]
    fn test_compute_quote_rejects_missing_relay_len() {
        let qb = make_test_quote_body();
        let ci = make_test_chain_info();
        // Exactly MIN_DATA_LEN (70 bytes) but request_bytes_len points past end
        let mut data = Vec::new();
        data.extend_from_slice(&2u16.to_le_bytes());
        data.extend_from_slice(&[0xab; 32]);
        data.extend_from_slice(&[0xcd; 32]);
        data.extend_from_slice(&10u32.to_le_bytes()); // request_bytes_len = 10
                                                      // No room for relay_instructions_len after skipping 10 request bytes
        assert!(compute_quote(&qb, &ci, &data).is_err());
    }
}

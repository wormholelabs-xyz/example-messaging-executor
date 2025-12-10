#![no_std]

extern crate alloc;
use alloc::vec::Vec;

// Request type prefixes
const REQ_VAA_V1: &[u8; 4] = b"ERV1";
const REQ_NTT_V1: &[u8; 4] = b"ERN1";
const REQ_CCTP_V1: &[u8; 4] = b"ERC1";
const REQ_CCTP_V2: &[u8; 4] = b"ERC2";

/// Encodes a version 1 VAA request payload.
pub fn make_vaa_v1_request(chain: u16, address: [u8; 32], sequence: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity({
        4 // type
        + 2 // chain
        + 32 // address
        + 8 // sequence
    });
    out.extend_from_slice(REQ_VAA_V1);
    out.extend_from_slice(&chain.to_be_bytes());
    out.extend_from_slice(&address);
    out.extend_from_slice(&sequence.to_be_bytes());
    out
}

/// Encodes a version 1 NTT request payload.
pub fn make_ntt_v1_request(
    source_chain: u16,
    source_manager: [u8; 32],
    message_id: [u8; 32],
) -> Vec<u8> {
    let mut out = Vec::with_capacity({
        4 // type
        + 2 // source chain
        + 32 // source_manager
        + 32 // message_id
    });
    out.extend_from_slice(REQ_NTT_V1);
    out.extend_from_slice(&source_chain.to_be_bytes());
    out.extend_from_slice(&source_manager);
    out.extend_from_slice(&message_id);
    out
}

/// Encodes a version 1 CCTP request payload.
pub fn make_cctp_v1_request(source_domain: u32, nonce: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity({
        4 // type
        + 4 // source domain
        + 8 // nonce
    });
    out.extend_from_slice(REQ_CCTP_V1);
    out.extend_from_slice(&source_domain.to_be_bytes());
    out.extend_from_slice(&nonce.to_be_bytes());
    out
}

/// Encodes a version 2 CCTP request payload.
/// This request currently assumes the Executor will auto detect the event off chain.
/// That may change in the future, in which case this interface would change.
pub fn make_cctp_v2_request() -> Vec<u8> {
    let mut out = Vec::with_capacity({
        4 // type
        + 1 // discovery
    });
    out.extend_from_slice(REQ_CCTP_V2);
    out.extend_from_slice(&[1]); // auto discovery
    out
}

// ============================================================================
// Relay Instructions
// ============================================================================
//
// Relay instructions tell the executor how to relay a message. The format
// matches the Wormhole SDK `relayInstructionsLayout` from
// @wormhole-foundation/sdk-definitions.
//
// Instructions are concatenated together. Each instruction starts with a
// 1-byte type discriminator followed by type-specific data. All multi-byte
// integers are big-endian.

/// Relay instruction type discriminators
pub const RELAY_IX_GAS: u8 = 1;
pub const RELAY_IX_GAS_DROP_OFF: u8 = 2;

/// Encodes a GasInstruction relay instruction.
///
/// Layout (33 bytes):
/// - type: u8 = 1
/// - gas_limit: u128 be (16 bytes)
/// - msg_value: u128 be (16 bytes)
pub fn make_relay_instruction_gas(gas_limit: u128, msg_value: u128) -> Vec<u8> {
    let mut out = Vec::with_capacity(33);
    out.push(RELAY_IX_GAS);
    out.extend_from_slice(&gas_limit.to_be_bytes());
    out.extend_from_slice(&msg_value.to_be_bytes());
    out
}

/// Encodes a GasDropOffInstruction relay instruction.
///
/// Layout (49 bytes):
/// - type: u8 = 2
/// - drop_off: u128 be (16 bytes)
/// - recipient: [u8; 32] (universal address)
pub fn make_relay_instruction_gas_drop_off(drop_off: u128, recipient: &[u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(49);
    out.push(RELAY_IX_GAS_DROP_OFF);
    out.extend_from_slice(&drop_off.to_be_bytes());
    out.extend_from_slice(recipient);
    out
}

/// Builder for constructing relay instructions.
///
/// Multiple instructions can be combined by appending them together.
/// This is a convenience wrapper that allows chaining.
#[derive(Default)]
pub struct RelayInstructionsBuilder {
    data: Vec<u8>,
}

impl RelayInstructionsBuilder {
    pub fn new() -> Self {
        Self { data: Vec::new() }
    }

    /// Add a GasInstruction to the relay instructions.
    pub fn with_gas(mut self, gas_limit: u128, msg_value: u128) -> Self {
        self.data.extend(make_relay_instruction_gas(gas_limit, msg_value));
        self
    }

    /// Add a GasDropOffInstruction to the relay instructions.
    pub fn with_gas_drop_off(mut self, drop_off: u128, recipient: &[u8; 32]) -> Self {
        self.data.extend(make_relay_instruction_gas_drop_off(drop_off, recipient));
        self
    }

    /// Build the final relay instructions bytes.
    pub fn build(self) -> Vec<u8> {
        self.data
    }
}

// ============================================================================
// Relay Instruction Parsing
// ============================================================================

/// Relay instruction parsing errors.
///
/// Discriminants are ordered to align with executor-quoter error codes (base 0x1002):
/// - 0 -> UnsupportedInstruction (0x1002)
/// - 1 -> MoreThanOneDropOff (0x1003)
/// - 2 -> MathOverflow (0x1004)
/// - 3 -> InvalidRelayInstructions (0x1005)
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelayParseError {
    /// Unknown relay instruction type
    UnsupportedType = 0,
    /// More than one drop-off instruction found
    MultipleDropoff = 1,
    /// Arithmetic overflow when accumulating gas_limit or msg_value
    Overflow = 2,
    /// Instruction data truncated / not enough bytes
    Truncated = 3,
}

/// Parses relay instructions to extract total gas limit and msg value.
///
/// Returns `(gas_limit, msg_value)` on success, or `RelayParseError` on failure.
/// Multiple gas instructions are summed. Only one dropoff is allowed.
///
/// Instruction format:
/// - Type 1 (Gas): 1 byte type + 16 bytes gas_limit + 16 bytes msg_value = 33 bytes
/// - Type 2 (DropOff): 1 byte type + 16 bytes msg_value + 32 bytes recipient = 49 bytes
pub fn parse_relay_instructions(data: &[u8]) -> Result<(u128, u128), RelayParseError> {
    let mut offset = 0;
    let mut gas_limit: u128 = 0;
    let mut msg_value: u128 = 0;
    let mut has_drop_off = false;

    while offset < data.len() {
        let ix_type = data[offset];
        offset += 1;

        match ix_type {
            RELAY_IX_GAS => {
                // Gas instruction: 16 bytes gas_limit + 16 bytes msg_value
                if offset + 32 > data.len() {
                    return Err(RelayParseError::Truncated);
                }

                let mut ix_gas_bytes = [0u8; 16];
                ix_gas_bytes.copy_from_slice(&data[offset..offset + 16]);
                let ix_gas_limit = u128::from_be_bytes(ix_gas_bytes);
                offset += 16;

                let mut ix_val_bytes = [0u8; 16];
                ix_val_bytes.copy_from_slice(&data[offset..offset + 16]);
                let ix_msg_value = u128::from_be_bytes(ix_val_bytes);
                offset += 16;

                gas_limit = gas_limit
                    .checked_add(ix_gas_limit)
                    .ok_or(RelayParseError::Overflow)?;
                msg_value = msg_value
                    .checked_add(ix_msg_value)
                    .ok_or(RelayParseError::Overflow)?;
            }
            RELAY_IX_GAS_DROP_OFF => {
                if has_drop_off {
                    return Err(RelayParseError::MultipleDropoff);
                }
                has_drop_off = true;

                // DropOff instruction: 16 bytes msg_value + 32 bytes recipient
                if offset + 48 > data.len() {
                    return Err(RelayParseError::Truncated);
                }

                let mut ix_val_bytes = [0u8; 16];
                ix_val_bytes.copy_from_slice(&data[offset..offset + 16]);
                let ix_msg_value = u128::from_be_bytes(ix_val_bytes);
                offset += 48; // Skip msg_value (16) + recipient (32)

                msg_value = msg_value
                    .checked_add(ix_msg_value)
                    .ok_or(RelayParseError::Overflow)?;
            }
            _ => {
                return Err(RelayParseError::UnsupportedType);
            }
        }
    }

    Ok((gas_limit, msg_value))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn test_vaa_v1() {
        let result = make_vaa_v1_request(
            10002,
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd4, 0xa6,
                0xa7, 0x2a, 0x02, 0x55, 0x99, 0xfd, 0x73, 0x57, 0xc0, 0xf1, 0x57, 0xc7, 0x18, 0xd0,
                0xf5, 0xe3, 0x8c, 0x76,
            ],
            29,
        );
        assert_eq!(
            result,
            [
                0x45, 0x52, 0x56, 0x31, 0x27, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0xd4, 0xa6, 0xa7, 0x2a, 0x02, 0x55, 0x99, 0xfd, 0x73, 0x57,
                0xc0, 0xf1, 0x57, 0xc7, 0x18, 0xd0, 0xf5, 0xe3, 0x8c, 0x76, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x1d
            ]
        );
    }

    #[test]
    fn test_ntt_v1() {
        let mut sequence: [u8; 32] = [0; 32];
        sequence[24..].copy_from_slice(&29_u64.to_be_bytes());
        let result = make_ntt_v1_request(
            10002,
            [
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xd4, 0xa6,
                0xa7, 0x2a, 0x02, 0x55, 0x99, 0xfd, 0x73, 0x57, 0xc0, 0xf1, 0x57, 0xc7, 0x18, 0xd0,
                0xf5, 0xe3, 0x8c, 0x76,
            ],
            sequence,
        );
        assert_eq!(
            result,
            [
                0x45, 0x52, 0x4E, 0x31, 0x27, 0x12, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0xd4, 0xa6, 0xa7, 0x2a, 0x02, 0x55, 0x99, 0xfd, 0x73, 0x57,
                0xc0, 0xf1, 0x57, 0xc7, 0x18, 0xd0, 0xf5, 0xe3, 0x8c, 0x76, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x1d
            ]
        );
    }

    #[test]
    fn test_cctp_v1() {
        let result = make_cctp_v1_request(6, 6344);
        assert_eq!(
            result,
            [
                0x45, 0x52, 0x43, 0x31, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x18, 0xc8
            ]
        );
    }

    #[test]
    fn test_cctp_v2() {
        let result = make_cctp_v2_request();
        assert_eq!(result, [0x45, 0x52, 0x43, 0x32, 0x01]);
    }

    #[test]
    fn test_relay_instruction_gas() {
        // GasInstruction with gasLimit=250_000 and msgValue=1_000_000
        let result = make_relay_instruction_gas(250_000, 1_000_000);
        assert_eq!(result.len(), 33);
        assert_eq!(result[0], RELAY_IX_GAS); // type = 1

        // gas_limit: 250_000 = 0x3D090 as u128 big-endian (16 bytes)
        let expected_gas_limit: [u8; 16] = 250_000u128.to_be_bytes();
        assert_eq!(&result[1..17], &expected_gas_limit);

        // msg_value: 1_000_000 = 0xF4240 as u128 big-endian (16 bytes)
        let expected_msg_value: [u8; 16] = 1_000_000u128.to_be_bytes();
        assert_eq!(&result[17..33], &expected_msg_value);
    }

    #[test]
    fn test_relay_instruction_gas_drop_off() {
        let recipient = [0xAB; 32];
        let result = make_relay_instruction_gas_drop_off(500_000, &recipient);
        assert_eq!(result.len(), 49);
        assert_eq!(result[0], RELAY_IX_GAS_DROP_OFF); // type = 2

        // drop_off: 500_000 as u128 big-endian (16 bytes)
        let expected_drop_off: [u8; 16] = 500_000u128.to_be_bytes();
        assert_eq!(&result[1..17], &expected_drop_off);

        // recipient: 32 bytes
        assert_eq!(&result[17..49], &recipient);
    }

    #[test]
    fn test_relay_instructions_builder() {
        let recipient = [0xCD; 32];
        let result = RelayInstructionsBuilder::new()
            .with_gas(100_000, 200_000)
            .with_gas_drop_off(300_000, &recipient)
            .build();

        // Total: 33 + 49 = 82 bytes
        assert_eq!(result.len(), 82);

        // First instruction: GasInstruction
        assert_eq!(result[0], RELAY_IX_GAS);
        assert_eq!(&result[1..17], &100_000u128.to_be_bytes());
        assert_eq!(&result[17..33], &200_000u128.to_be_bytes());

        // Second instruction: GasDropOffInstruction
        assert_eq!(result[33], RELAY_IX_GAS_DROP_OFF);
        assert_eq!(&result[34..50], &300_000u128.to_be_bytes());
        assert_eq!(&result[50..82], &recipient);
    }

    #[test]
    fn test_relay_instructions_builder_empty() {
        let result = RelayInstructionsBuilder::new().build();
        assert_eq!(result.len(), 0);
    }

    // ========================================================================
    // parse_relay_instructions tests
    // ========================================================================

    #[test]
    fn test_parse_relay_instructions_empty() {
        let result = parse_relay_instructions(&[]);
        assert_eq!(result, Ok((0, 0)));
    }

    #[test]
    fn test_parse_relay_instructions_gas() {
        let data = make_relay_instruction_gas(250_000, 1_000_000);
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Ok((250_000, 1_000_000)));
    }

    #[test]
    fn test_parse_relay_instructions_dropoff() {
        let recipient = [0xAB; 32];
        let data = make_relay_instruction_gas_drop_off(500_000, &recipient);
        let result = parse_relay_instructions(&data);
        // DropOff contributes to msg_value, not gas_limit
        assert_eq!(result, Ok((0, 500_000)));
    }

    #[test]
    fn test_parse_relay_instructions_gas_and_dropoff() {
        let recipient = [0xCD; 32];
        let data = RelayInstructionsBuilder::new()
            .with_gas(100_000, 200_000)
            .with_gas_drop_off(300_000, &recipient)
            .build();
        let result = parse_relay_instructions(&data);
        // gas_limit = 100_000, msg_value = 200_000 + 300_000 = 500_000
        assert_eq!(result, Ok((100_000, 500_000)));
    }

    #[test]
    fn test_parse_relay_instructions_multiple_gas() {
        let mut data = make_relay_instruction_gas(100_000, 50_000);
        data.extend(make_relay_instruction_gas(200_000, 75_000));
        data.extend(make_relay_instruction_gas(50_000, 25_000));
        let result = parse_relay_instructions(&data);
        // gas_limit = 100k + 200k + 50k = 350k
        // msg_value = 50k + 75k + 25k = 150k
        assert_eq!(result, Ok((350_000, 150_000)));
    }

    #[test]
    fn test_parse_relay_instructions_invalid_type() {
        let data = [0xFF, 0x00, 0x00]; // Invalid type 0xFF
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Err(RelayParseError::UnsupportedType));
    }

    #[test]
    fn test_parse_relay_instructions_truncated_gas() {
        // Gas instruction needs 33 bytes (1 type + 16 gas_limit + 16 msg_value)
        // Provide only 10 bytes after type
        let mut data = vec![RELAY_IX_GAS];
        data.extend_from_slice(&[0u8; 10]);
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Err(RelayParseError::Truncated));
    }

    #[test]
    fn test_parse_relay_instructions_truncated_dropoff() {
        // DropOff instruction needs 49 bytes (1 type + 16 msg_value + 32 recipient)
        // Provide only 20 bytes after type
        let mut data = vec![RELAY_IX_GAS_DROP_OFF];
        data.extend_from_slice(&[0u8; 20]);
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Err(RelayParseError::Truncated));
    }

    #[test]
    fn test_parse_relay_instructions_multiple_dropoff() {
        let recipient = [0xAB; 32];
        let mut data = make_relay_instruction_gas_drop_off(100_000, &recipient);
        data.extend(make_relay_instruction_gas_drop_off(200_000, &recipient));
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Err(RelayParseError::MultipleDropoff));
    }

    #[test]
    fn test_parse_relay_instructions_overflow_gas_limit() {
        let mut data = make_relay_instruction_gas(u128::MAX, 0);
        data.extend(make_relay_instruction_gas(1, 0)); // This should overflow
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Err(RelayParseError::Overflow));
    }

    #[test]
    fn test_parse_relay_instructions_overflow_msg_value() {
        let mut data = make_relay_instruction_gas(0, u128::MAX);
        data.extend(make_relay_instruction_gas(0, 1)); // This should overflow
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Err(RelayParseError::Overflow));
    }

    // Roundtrip tests

    #[test]
    fn test_roundtrip_gas() {
        let gas_limit = 1_000_000u128;
        let msg_value = 2_000_000_000_000_000_000u128; // 2 ETH in wei
        let data = make_relay_instruction_gas(gas_limit, msg_value);
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Ok((gas_limit, msg_value)));
    }

    #[test]
    fn test_roundtrip_dropoff() {
        let drop_off = 500_000_000_000_000_000u128; // 0.5 ETH in wei
        let recipient = [0x42; 32];
        let data = make_relay_instruction_gas_drop_off(drop_off, &recipient);
        let result = parse_relay_instructions(&data);
        assert_eq!(result, Ok((0, drop_off)));
    }

    #[test]
    fn test_roundtrip_builder() {
        let gas_limit = 300_000u128;
        let gas_msg_value = 100_000_000_000_000_000u128; // 0.1 ETH
        let drop_off = 250_000_000_000_000_000u128; // 0.25 ETH
        let recipient = [0x99; 32];

        let data = RelayInstructionsBuilder::new()
            .with_gas(gas_limit, gas_msg_value)
            .with_gas_drop_off(drop_off, &recipient)
            .build();

        let result = parse_relay_instructions(&data);
        assert_eq!(result, Ok((gas_limit, gas_msg_value + drop_off)));
    }
}

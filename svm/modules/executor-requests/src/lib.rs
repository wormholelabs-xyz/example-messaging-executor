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

#[cfg(test)]
mod tests {
    use super::*;

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
}

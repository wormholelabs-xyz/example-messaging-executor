//! CPI instruction builder for executor program.
//!
//! The quoter CPI now uses zero-copy: instruction data is passed directly from the router's
//! input without reconstruction. Only the executor CPI requires building instruction data
//! because the signed_quote is constructed on-chain.

extern crate alloc;
use alloc::vec::Vec;

/// Anchor discriminator for executor::request_for_execution
/// Generated from: sha256("global:request_for_execution")[0..8]
const EXECUTOR_REQUEST_FOR_EXECUTION_DISCRIMINATOR: [u8; 8] = [0x6d, 0x6b, 0x57, 0x25, 0x97, 0xc0, 0x77, 0x73];

/// Builds instruction data for Anchor executor::request_for_execution CPI.
///
/// The executor program uses Anchor serialization (Borsh):
/// - 8-byte discriminator
/// - RequestForExecutionArgs struct (Borsh-serialized)
///
/// RequestForExecutionArgs layout:
/// - amount: u64 (8 bytes, little-endian)
/// - dst_chain: u16 (2 bytes, little-endian)
/// - dst_addr: [u8; 32] (32 bytes)
/// - refund_addr: Pubkey (32 bytes)
/// - signed_quote_bytes: Vec<u8> (4-byte length prefix + data)
/// - request_bytes: Vec<u8> (4-byte length prefix + data)
/// - relay_instructions: Vec<u8> (4-byte length prefix + data)
///
/// Note: This function still allocates because the signed_quote is constructed on-chain
/// and must be combined with other fields into a contiguous buffer.
pub fn make_executor_request_for_execution_ix(
    amount: u64,
    dst_chain: u16,
    dst_addr: &[u8; 32],
    refund_addr: &[u8; 32],
    signed_quote_bytes: &[u8],
    request_bytes: &[u8],
    relay_instructions: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity({
        8 // discriminator
        + 8 // amount
        + 2 // dst_chain
        + 32 // dst_addr
        + 32 // refund_addr
        + 4 + signed_quote_bytes.len() // signed_quote_bytes Vec
        + 4 + request_bytes.len() // request_bytes Vec
        + 4 + relay_instructions.len() // relay_instructions Vec
    });

    // Anchor discriminator
    out.extend_from_slice(&EXECUTOR_REQUEST_FOR_EXECUTION_DISCRIMINATOR);

    // RequestForExecutionArgs (Borsh serialization - all little-endian)
    out.extend_from_slice(&amount.to_le_bytes());
    out.extend_from_slice(&dst_chain.to_le_bytes());
    out.extend_from_slice(dst_addr);
    out.extend_from_slice(refund_addr);

    // Vec<u8> in Borsh: 4-byte length prefix (u32 le) + data
    out.extend_from_slice(&(signed_quote_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(signed_quote_bytes);

    out.extend_from_slice(&(request_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(request_bytes);

    out.extend_from_slice(&(relay_instructions.len() as u32).to_le_bytes());
    out.extend_from_slice(relay_instructions);

    out
}

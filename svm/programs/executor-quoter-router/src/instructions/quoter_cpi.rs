//! CPI instruction builders for executor-quoter and executor programs.
//!
//! These functions build instruction data for CPI calls to the quoter and executor implementations.

extern crate alloc;
use alloc::vec::Vec;

/// Executor-quoter instruction discriminators
const IX_QUOTER_REQUEST_QUOTE: u8 = 3;
const IX_QUOTER_REQUEST_EXECUTION_QUOTE: u8 = 4;

/// Anchor discriminator for executor::request_for_execution
/// Generated from: sha256("global:request_for_execution")[0..8]
const EXECUTOR_REQUEST_FOR_EXECUTION_DISCRIMINATOR: [u8; 8] = [0x6d, 0x6b, 0x57, 0x25, 0x97, 0xc0, 0x77, 0x73];

/// Builds instruction data for executor-quoter RequestQuote CPI.
///
/// Layout:
/// - discriminator: u8 (1 byte)
/// - dst_chain: u16 le (2 bytes)
/// - dst_addr: [u8; 32] (32 bytes)
/// - refund_addr: [u8; 32] (32 bytes)
/// - request_bytes_len: u32 le (4 bytes)
/// - request_bytes: [u8; request_bytes_len]
/// - relay_instructions_len: u32 le (4 bytes)
/// - relay_instructions: [u8; relay_instructions_len]
pub fn make_quoter_request_quote_ix(
    dst_chain: u16,
    dst_addr: &[u8; 32],
    refund_addr: &[u8; 32],
    request_bytes: &[u8],
    relay_instructions: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity({
        1 // discriminator
        + 2 // dst_chain
        + 32 // dst_addr
        + 32 // refund_addr
        + 4 // request_bytes_len
        + request_bytes.len()
        + 4 // relay_instructions_len
        + relay_instructions.len()
    });
    out.push(IX_QUOTER_REQUEST_QUOTE);
    out.extend_from_slice(&dst_chain.to_le_bytes());
    out.extend_from_slice(dst_addr);
    out.extend_from_slice(refund_addr);
    out.extend_from_slice(&(request_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(request_bytes);
    out.extend_from_slice(&(relay_instructions.len() as u32).to_le_bytes());
    out.extend_from_slice(relay_instructions);
    out
}

/// Builds instruction data for executor-quoter RequestExecutionQuote CPI.
///
/// Layout: Same as RequestQuote but with different discriminator.
pub fn make_quoter_request_execution_quote_ix(
    dst_chain: u16,
    dst_addr: &[u8; 32],
    refund_addr: &[u8; 32],
    request_bytes: &[u8],
    relay_instructions: &[u8],
) -> Vec<u8> {
    let mut out = Vec::with_capacity({
        1 // discriminator
        + 2 // dst_chain
        + 32 // dst_addr
        + 32 // refund_addr
        + 4 // request_bytes_len
        + request_bytes.len()
        + 4 // relay_instructions_len
        + relay_instructions.len()
    });
    out.push(IX_QUOTER_REQUEST_EXECUTION_QUOTE);
    out.extend_from_slice(&dst_chain.to_le_bytes());
    out.extend_from_slice(dst_addr);
    out.extend_from_slice(refund_addr);
    out.extend_from_slice(&(request_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(request_bytes);
    out.extend_from_slice(&(relay_instructions.len() as u32).to_le_bytes());
    out.extend_from_slice(relay_instructions);
    out
}

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

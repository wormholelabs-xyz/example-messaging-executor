//! Router-specific serialization for signed quotes (EQ02) and governance messages (EG01).
//! These are only used by the router program, not shared with other programs/clients.

use pinocchio::program_error::ProgramError;

use crate::error::ExecutorQuoterRouterError;

/// EQ02 signed quote prefix
pub const QUOTE_PREFIX_EQ02: &[u8; 4] = b"EQ02";

/// EG01 governance message prefix
pub const GOV_PREFIX_EG01: &[u8; 4] = b"EG01";

/// Constructs an EQ02 signed quote.
///
/// Layout (100 bytes):
/// - bytes 0-3:   prefix "EQ02" (4 bytes)
/// - bytes 4-23:  quoter_address (20 bytes, Ethereum address)
/// - bytes 24-55: payee_address (32 bytes, universal address)
/// - bytes 56-57: src_chain (u16 be)
/// - bytes 58-59: dst_chain (u16 be)
/// - bytes 60-67: expiry_time (u64 be)
/// - bytes 68-99: quote_body (32 bytes, EQ01 format)
pub fn make_signed_quote_eq02(
    quoter_address: &[u8; 20],
    payee_address: &[u8; 32],
    src_chain: u16,
    dst_chain: u16,
    expiry_time: u64,
    quote_body: &[u8; 32],
) -> [u8; 100] {
    let mut out = [0u8; 100];
    out[0..4].copy_from_slice(QUOTE_PREFIX_EQ02);
    out[4..24].copy_from_slice(quoter_address);
    out[24..56].copy_from_slice(payee_address);
    out[56..58].copy_from_slice(&src_chain.to_be_bytes());
    out[58..60].copy_from_slice(&dst_chain.to_be_bytes());
    out[60..68].copy_from_slice(&expiry_time.to_be_bytes());
    out[68..100].copy_from_slice(quote_body);
    out
}

/// Parsed governance message for UpdateQuoterContract.
///
/// Layout (163 bytes):
/// - bytes 0-3:    prefix "EG01" (4 bytes)
/// - bytes 4-5:    chain_id (u16 be)
/// - bytes 6-25:   quoter_address (20 bytes, Ethereum address)
/// - bytes 26-57:  universal_contract_address (32 bytes)
/// - bytes 58-89:  universal_sender_address (32 bytes)
/// - bytes 90-97:  expiry_time (u64 be)
/// - bytes 98-129: signature_r (32 bytes)
/// - bytes 130-161: signature_s (32 bytes)
/// - byte 162:     signature_v (1 byte)
#[derive(Debug, Clone, Copy)]
pub struct GovernanceMessage {
    pub chain_id: u16,
    pub quoter_address: [u8; 20],
    pub universal_contract_address: [u8; 32],
    pub universal_sender_address: [u8; 32],
    pub expiry_time: u64,
    pub signature_r: [u8; 32],
    pub signature_s: [u8; 32],
    pub signature_v: u8,
}

impl GovernanceMessage {
    pub const LEN: usize = 163;

    /// Parse a governance message from bytes.
    pub fn parse(data: &[u8]) -> Result<Self, ProgramError> {
        if data.len() < Self::LEN {
            return Err(ExecutorQuoterRouterError::InvalidInstructionData.into());
        }

        // Check prefix
        if &data[0..4] != GOV_PREFIX_EG01 {
            return Err(ExecutorQuoterRouterError::InvalidGovernancePrefix.into());
        }

        let mut chain_id_bytes = [0u8; 2];
        chain_id_bytes.copy_from_slice(&data[4..6]);
        let chain_id = u16::from_be_bytes(chain_id_bytes);

        let mut quoter_address = [0u8; 20];
        quoter_address.copy_from_slice(&data[6..26]);

        let mut universal_contract_address = [0u8; 32];
        universal_contract_address.copy_from_slice(&data[26..58]);

        let mut universal_sender_address = [0u8; 32];
        universal_sender_address.copy_from_slice(&data[58..90]);

        let mut expiry_time_bytes = [0u8; 8];
        expiry_time_bytes.copy_from_slice(&data[90..98]);
        let expiry_time = u64::from_be_bytes(expiry_time_bytes);

        let mut signature_r = [0u8; 32];
        signature_r.copy_from_slice(&data[98..130]);

        let mut signature_s = [0u8; 32];
        signature_s.copy_from_slice(&data[130..162]);

        let signature_v = data[162];

        Ok(Self {
            chain_id,
            quoter_address,
            universal_contract_address,
            universal_sender_address,
            expiry_time,
            signature_r,
            signature_s,
            signature_v,
        })
    }

    /// Get the message bytes that were signed (bytes 0-98).
    pub fn signed_message<'a>(&self, original_data: &'a [u8]) -> &'a [u8] {
        &original_data[0..98]
    }

}

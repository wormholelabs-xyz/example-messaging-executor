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
}

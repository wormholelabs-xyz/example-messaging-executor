// SPDX-License-Identifier: Apache-2.0

module executor_requests::executor_requests {
    use executor::bytes;

    const REQ_VAA_V1: vector<u8> = b"ERV1";
    const REQ_CCTP_V1: vector<u8> = b"ERC1";

    const E_INVALID_VEC_LENGTH: u64 = 0;

    public fun make_vaa_v1_request(
        emitter_chain: u16,
        emitter_address: vector<u8>,
        sequence: u64
    ): vector<u8> {
        assert!(emitter_address.length() == 32,E_INVALID_VEC_LENGTH);
        let mut ret = vector::empty();
        ret.append(REQ_VAA_V1);
        bytes::push_u16_be(&mut ret, emitter_chain);
        ret.append(emitter_address);
        bytes::push_u64_be(&mut ret, sequence);
        ret
    }

    public fun make_cctp_v1_request(
        src_domain: u32,
        nonce: u64,
    ): vector<u8> {
        let mut ret = vector::empty();
        ret.append(REQ_CCTP_V1);
        bytes::push_u32_be(&mut ret, src_domain);
        bytes::push_u64_be(&mut ret, nonce);
        ret
    }
}

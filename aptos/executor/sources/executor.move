// SPDX-License-Identifier: Apache-2.0

module executor::executor {
    use aptos_framework::aptos_coin::{AptosCoin};
    use aptos_framework::coin::{Self, Coin};
    use aptos_framework::event;
    use aptos_std::from_bcs;
    use executor::bytes;
    use executor::cursor;

    const CHAIN_ID: u16 = 22;

    const E_QUOTE_SRC_CHAIN_MISMATCH: u64 = 0;
    const E_QUOTE_DST_CHAIN_MISMATCH: u64 = 1;
    const E_QUOTE_EXPIRED: u64 = 2;

    #[event]
    struct RequestForExecution has drop, store {
        quoter_address: vector<u8>,
        amt_paid: u64,
        dst_chain: u16,
        dst_addr: address,
        refund_addr: address,
        signed_quote: vector<u8>,
        request_bytes: vector<u8>,
        relay_instructions: vector<u8>,
    }

    public fun request_execution(
        amount: Coin<AptosCoin>,
        dst_chain: u16, 
        dst_addr: address, // akin to bytes32 
        refund_addr: address, 
        signed_quote_bytes: vector<u8>, 
        request_bytes: vector<u8>, 
        relay_instructions: vector<u8>
    ) {
        let cursor = cursor::new(signed_quote_bytes);
        bytes::take_bytes(&mut cursor, 4); // prefix
        let quoter_address = bytes::take_bytes(&mut cursor, 20);
        let payee_address = from_bcs::to_address(bytes::take_bytes(&mut cursor, 32));
        let quote_src_chain = bytes::take_u16_be(&mut cursor);
        assert!(quote_src_chain == CHAIN_ID, E_QUOTE_SRC_CHAIN_MISMATCH);
        let quote_dst_chain = bytes::take_u16_be(&mut cursor);
        assert!(quote_dst_chain == dst_chain, E_QUOTE_DST_CHAIN_MISMATCH);
        let expiry_time = bytes::take_u64_be(&mut cursor);
        assert!(expiry_time > aptos_framework::timestamp::now_seconds(), E_QUOTE_EXPIRED);
        cursor::take_rest(cursor);
        let amt_paid = coin::value<AptosCoin>(&amount);
        coin::deposit(payee_address, amount);
        event::emit(RequestForExecution {
            quoter_address,
            amt_paid,
            dst_chain,
            dst_addr,
            refund_addr,
            signed_quote: signed_quote_bytes,
            request_bytes,
            relay_instructions
        });
    }
}

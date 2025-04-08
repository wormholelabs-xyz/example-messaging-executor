// SPDX-License-Identifier: Apache-2.0

module executor::executor {
    use executor::bytes;
    use executor::cursor;
    use sui::clock::Clock;
    use sui::coin::{Coin};
    use sui::event;
    use sui::sui::{SUI};

    const CHAIN_ID: u16 = 21;

    const E_QUOTE_SRC_CHAIN_MISMATCH: u64 = 0;
    const E_QUOTE_DST_CHAIN_MISMATCH: u64 = 1;
    const E_QUOTE_EXPIRED: u64 = 2;

    public struct RequestForExecution has copy, drop {
        quoter_address: vector<u8>,
        amt_paid: u64,
        dst_chain: u16,
        dst_addr: vector<u8>,
        refund_addr: address,
        signed_quote: vector<u8>,
        request_bytes: vector<u8>,
        relay_instructions: vector<u8>,
    }

    public fun request_execution(
        amount: Coin<SUI>,
        clock: &Clock,
        dst_chain: u16, 
        dst_addr: vector<u8>, 
        refund_addr: address, 
        signed_quote_bytes: vector<u8>, 
        request_bytes: vector<u8>, 
        relay_instructions: vector<u8>
    ) {
        let mut cursor = cursor::new(signed_quote_bytes);
        bytes::take_bytes(&mut cursor, 4); // prefix
        let quoter_address = bytes::take_bytes(&mut cursor, 20);
        let payee_address = sui::address::from_bytes(bytes::take_bytes(&mut cursor, 32));
        let quote_src_chain = bytes::take_u16_be(&mut cursor);
        assert!(quote_src_chain == CHAIN_ID, E_QUOTE_SRC_CHAIN_MISMATCH);
        let quote_dst_chain = bytes::take_u16_be(&mut cursor);
        assert!(quote_dst_chain == dst_chain, E_QUOTE_DST_CHAIN_MISMATCH);
        let expiry_time = bytes::take_u64_be(&mut cursor);
        assert!(expiry_time > clock.timestamp_ms() / 1000, E_QUOTE_EXPIRED);
        cursor.take_rest();
        let amt_paid = amount.value();
        transfer::public_transfer(amount, payee_address);
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

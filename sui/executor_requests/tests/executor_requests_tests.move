// SPDX-License-Identifier: Apache-2.0

#[test_only]
module executor_requests::executor_requests_tests {
    use executor_requests::executor_requests;

    #[test]
    fun test_make_vaa_v1_request() {
        let res = executor_requests::make_vaa_v1_request(
            10002,
            x"000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c76",
            29
        );
        assert!(res == x"455256312712000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c76000000000000001d", 0);
    }

    #[test]
    fun test_make_ntt_v1_request() {
        let res = executor_requests::make_ntt_v1_request(
            10002,
            x"000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c76",
            x"0000000000000000000000000000000000000000000000000000000000000001",
        );
        assert!(res == x"45524E312712000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c760000000000000000000000000000000000000000000000000000000000000001", 0);
    }

    #[test]
    fun test_make_cctp_v1_request() {
        let res = executor_requests::make_cctp_v1_request(
            6,
            6344
        );
        assert!(res == x"455243310000000600000000000018c8", 0);
    }

    #[test]
    fun test_make_cctp_v2_request() {
        let res = executor_requests::make_cctp_v2_request();
        assert!(res == x"4552433201", 0);
    }

    #[test]
    #[expected_failure(abort_code = executor_requests::E_INVALID_VEC_LENGTH)]
    fun test_make_vaa_v1_request_fail_with_emitter_too_short() {
        executor_requests::make_vaa_v1_request(
            10002,
            x"000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c",
            29
        );
    }

    #[test]
    #[expected_failure(abort_code = executor_requests::E_INVALID_VEC_LENGTH)]
    fun test_make_vaa_v1_request_fail_with_emitter_too_long() {
        executor_requests::make_vaa_v1_request(
            10002,
            x"000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c7600",
            29
        );
    }

    #[test]
    #[expected_failure(abort_code = executor_requests::E_INVALID_VEC_LENGTH)]
    fun test_make_ntt_v1_request_fail_with_address_too_short() {
        executor_requests::make_ntt_v1_request(
            10002,
            x"000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c",
            x"0000000000000000000000000000000000000000000000000000000000000001"
        );
    }

    #[test]
    #[expected_failure(abort_code = executor_requests::E_INVALID_VEC_LENGTH)]
    fun test_make_ntt_v1_request_fail_with_address_too_long() {
        executor_requests::make_ntt_v1_request(
            10002,
            x"000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c7600",
            x"0000000000000000000000000000000000000000000000000000000000000001"
        );
    }

    #[test]
    #[expected_failure(abort_code = executor_requests::E_INVALID_VEC_LENGTH)]
    fun test_make_ntt_v1_request_fail_with_message_id_too_short() {
        executor_requests::make_ntt_v1_request(
            10002,
            x"000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c76",
            x"00000000000000000000000000000000000000000000000000000000000000"
        );
    }

    #[test]
    #[expected_failure(abort_code = executor_requests::E_INVALID_VEC_LENGTH)]
    fun test_make_ntt_v1_request_fail_with_message_id_too_long() {
        executor_requests::make_ntt_v1_request(
            10002,
            x"000000000000000000000000d4a6a72a025599fd7357c0f157c718d0f5e38c76",
            x"000000000000000000000000000000000000000000000000000000000000000100"
        );
    }
    
}

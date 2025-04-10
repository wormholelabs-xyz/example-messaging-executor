// SPDX-License-Identifier: Apache-2.0

#[test_only]
module executor::executor_tests {
    use executor::executor;
    use sui::clock;
    use sui::coin::{Self, Coin};
    use sui::sui::{SUI};
    use sui::test_scenario;

    #[test]
    fun test_executor() {
        let user = @0xCAFE;
        let mut my_scenario = test_scenario::begin(user);
        let scenario = &mut my_scenario;
        let the_clock = clock::create_for_testing(test_scenario::ctx(scenario));
        let amount = coin::mint_for_testing(
            100,
            test_scenario::ctx(scenario)
        );
        let expected_payee = @0x000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a;
        assert!(test_scenario::most_recent_id_for_address<Coin<SUI>>(expected_payee).is_none(), 0);
        executor::request_execution(
            amount,
            &the_clock,
            6,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a001500060000000067dc8c0a00000000000003e800000000000000020000120430544c000000002bb3cab500199f0dc83362b168ade5aa32a99747a0d0b4d7c7d7f9acdee2c62b89de7661dc6968d845bf9c0997553f6f9c0418ba3cee97787eb9c19266754892124c8c65751b", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        let effects = test_scenario::next_tx(scenario, user);
        assert!(test_scenario::num_user_events(&effects) == 1, 0);
        assert!(test_scenario::most_recent_id_for_address<Coin<SUI>>(expected_payee).is_some(), 0);
        clock::destroy_for_testing(the_clock);
        test_scenario::end(my_scenario);
    }

    #[test]
    #[expected_failure]
    fun test_executor_fail_with_invalid_quote_header() {
        let user = @0xCAFE;
        let mut my_scenario = test_scenario::begin(user);
        let scenario = &mut my_scenario;
        let the_clock = clock::create_for_testing(test_scenario::ctx(scenario));
        executor::request_execution(
            coin::mint_for_testing(
                100,
                test_scenario::ctx(scenario)
            ),
            &the_clock,
            6,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a001500060000000067dc8c", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        let effects = test_scenario::next_tx(scenario, user);
        assert!(test_scenario::num_user_events(&effects) == 1, 0);
        clock::destroy_for_testing(the_clock);
        test_scenario::end(my_scenario);
    }

    #[test]
    #[expected_failure(abort_code = executor::E_QUOTE_SRC_CHAIN_MISMATCH)]
    fun test_executor_fail_with_invalid_source_chain() {
        let user = @0xCAFE;
        let mut my_scenario = test_scenario::begin(user);
        let scenario = &mut my_scenario;
        let the_clock = clock::create_for_testing(test_scenario::ctx(scenario));
        executor::request_execution(
            coin::mint_for_testing(
                100,
                test_scenario::ctx(scenario)
            ),
            &the_clock,
            6,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a000600060000000067dc8c0a00000000000003e800000000000000020000120430544c000000002bb3cab500199f0dc83362b168ade5aa32a99747a0d0b4d7c7d7f9acdee2c62b89de7661dc6968d845bf9c0997553f6f9c0418ba3cee97787eb9c19266754892124c8c65751b", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        let effects = test_scenario::next_tx(scenario, user);
        assert!(test_scenario::num_user_events(&effects) == 1, 0);
        clock::destroy_for_testing(the_clock);
        test_scenario::end(my_scenario);
    }

    #[test]
    #[expected_failure(abort_code = executor::E_QUOTE_DST_CHAIN_MISMATCH)]
    fun test_executor_fail_with_invalid_destination_chain() {
        let user = @0xCAFE;
        let mut my_scenario = test_scenario::begin(user);
        let scenario = &mut my_scenario;
        let the_clock = clock::create_for_testing(test_scenario::ctx(scenario));
        executor::request_execution(
            coin::mint_for_testing(
                100,
                test_scenario::ctx(scenario)
            ),
            &the_clock,
            10002,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a001500060000000067dc8c0a00000000000003e800000000000000020000120430544c000000002bb3cab500199f0dc83362b168ade5aa32a99747a0d0b4d7c7d7f9acdee2c62b89de7661dc6968d845bf9c0997553f6f9c0418ba3cee97787eb9c19266754892124c8c65751b", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        let effects = test_scenario::next_tx(scenario, user);
        assert!(test_scenario::num_user_events(&effects) == 1, 0);
        clock::destroy_for_testing(the_clock);
        test_scenario::end(my_scenario);
    }

    #[test]
    #[expected_failure(abort_code = executor::E_QUOTE_EXPIRED)]
    fun test_executor_fail_with_expired_quote() {
        let user = @0xCAFE;
        let mut my_scenario = test_scenario::begin(user);
        let scenario = &mut my_scenario;
        let mut the_clock = clock::create_for_testing(test_scenario::ctx(scenario));
        clock::set_for_testing(&mut the_clock, 1742507018*1000);
        executor::request_execution(
            coin::mint_for_testing(
                100,
                test_scenario::ctx(scenario)
            ),
            &the_clock,
            6,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a001500060000000067dc8c0a00000000000003e800000000000000020000120430544c000000002bb3cab500199f0dc83362b168ade5aa32a99747a0d0b4d7c7d7f9acdee2c62b89de7661dc6968d845bf9c0997553f6f9c0418ba3cee97787eb9c19266754892124c8c65751b", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        let effects = test_scenario::next_tx(scenario, user);
        assert!(test_scenario::num_user_events(&effects) == 1, 0);
        clock::destroy_for_testing(the_clock);
        test_scenario::end(my_scenario);
    }

}

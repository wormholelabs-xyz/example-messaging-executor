// SPDX-License-Identifier: Apache-2.0

#[test_only]
module executor::executor_tests {
    use executor::executor;
    use aptos_framework::aptos_coin::{Self, AptosCoin};
    use aptos_framework::coin;
    use aptos_framework::event;
    use std::signer;
    use std::vector;

    #[test(aptos_framework = @aptos_framework, expected_payee = @0x000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a)]
    fun test_executor(aptos_framework: &signer, expected_payee: &signer) {
        let user = @0xCAFE;
        aptos_framework::timestamp::set_time_has_started_for_testing(aptos_framework);
        let (burn_cap, mint_cap) = aptos_coin::initialize_for_test(aptos_framework);
        let amount = coin::mint(100, &mint_cap);
        // The payee account must exist to be registered for a coin
        std::account::create_account_for_test(signer::address_of(expected_payee));
        // The payee must be registered for the AptosCoin in order to receive a deposit
        coin::register<AptosCoin>(expected_payee);
        assert!(coin::balance<AptosCoin>(signer::address_of(expected_payee)) == 0, 0);
        executor::request_execution(
            amount,
            6,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a001600060000000067dc8c0a00000000000003e800000000000000020000120430544c000000002bb3cab500199f0dc83362b168ade5aa32a99747a0d0b4d7c7d7f9acdee2c62b89de7661dc6968d845bf9c0997553f6f9c0418ba3cee97787eb9c19266754892124c8c65751b", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        let effects = event::emitted_events<executor::RequestForExecution>();
        assert!(vector::length(&effects) == 1, 0);
        assert!(coin::balance<AptosCoin>(signer::address_of(expected_payee)) == 100, 0);
        coin::destroy_mint_cap(mint_cap);
        coin::destroy_burn_cap(burn_cap);
    }

    #[test(aptos_framework = @aptos_framework)]
    #[expected_failure]
    fun test_executor_fail_with_invalid_quote_header(aptos_framework: &signer) {
        let user = @0xCAFE;
        let (burn_cap, mint_cap) = aptos_coin::initialize_for_test(aptos_framework);
        executor::request_execution(
            coin::mint(100, &mint_cap),
            6,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a001600060000000067dc8c", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        coin::destroy_mint_cap(mint_cap);
        coin::destroy_burn_cap(burn_cap);
    }

    #[test(aptos_framework = @aptos_framework)]
    #[expected_failure(abort_code = executor::E_QUOTE_SRC_CHAIN_MISMATCH)]
    fun test_executor_fail_with_invalid_source_chain(aptos_framework: &signer) {
        let user = @0xCAFE;
        let (burn_cap, mint_cap) = aptos_coin::initialize_for_test(aptos_framework);
        executor::request_execution(
            coin::mint(100, &mint_cap),
            6,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a000600060000000067dc8c0a00000000000003e800000000000000020000120430544c000000002bb3cab500199f0dc83362b168ade5aa32a99747a0d0b4d7c7d7f9acdee2c62b89de7661dc6968d845bf9c0997553f6f9c0418ba3cee97787eb9c19266754892124c8c65751b", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        coin::destroy_mint_cap(mint_cap);
        coin::destroy_burn_cap(burn_cap);
    }

    #[test(aptos_framework = @aptos_framework)]
    #[expected_failure(abort_code = executor::E_QUOTE_DST_CHAIN_MISMATCH)]
    fun test_executor_fail_with_invalid_destination_chain(aptos_framework: &signer) {
        let user = @0xCAFE;
        let (burn_cap, mint_cap) = aptos_coin::initialize_for_test(aptos_framework);
        executor::request_execution(
            coin::mint(100, &mint_cap),
            10002,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a001600060000000067dc8c0a00000000000003e800000000000000020000120430544c000000002bb3cab500199f0dc83362b168ade5aa32a99747a0d0b4d7c7d7f9acdee2c62b89de7661dc6968d845bf9c0997553f6f9c0418ba3cee97787eb9c19266754892124c8c65751b", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        coin::destroy_mint_cap(mint_cap);
        coin::destroy_burn_cap(burn_cap);
    }

    #[test(aptos_framework = @aptos_framework)]
    #[expected_failure(abort_code = executor::E_QUOTE_EXPIRED)]
    fun test_executor_fail_with_expired_quote(aptos_framework: &signer) {
        let user = @0xCAFE;
        aptos_framework::timestamp::set_time_has_started_for_testing(aptos_framework);
        aptos_framework::timestamp::update_global_time_for_test_secs(1742507018);
        let (burn_cap, mint_cap) = aptos_coin::initialize_for_test(aptos_framework);
        executor::request_execution(
            coin::mint(100, &mint_cap),
            6,
            @0x1234567891234567891234567891234512345678912345678912345678912345,
            user,
            x"455130315241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000f7122c001b3e07d7fafd8be3670545135859954a001600060000000067dc8c0a00000000000003e800000000000000020000120430544c000000002bb3cab500199f0dc83362b168ade5aa32a99747a0d0b4d7c7d7f9acdee2c62b89de7661dc6968d845bf9c0997553f6f9c0418ba3cee97787eb9c19266754892124c8c65751b", 
            x"45524e312712000000000000000000000000e2a90da727f328e2324536fe2b4837f6c77dda7d0000000000000000000000000000000000000000000000000000000000000006",
            x"0100000000000000000000000000061a8000000000000000000000000000000000"
        );
        coin::destroy_mint_cap(mint_cap);
        coin::destroy_burn_cap(burn_cap);
    }

}

//! Compute unit benchmarks for executor-quoter-router using mollusk-svm.
//!
//! Run with: cargo bench
//! Output: target/benches/executor_quoter_router_compute_units.md
//!
//! Note: Only the Initialize instruction is benchmarked here.
//! - UpdateQuoterContract requires secp256k1 syscalls (sol_secp256k1_recover)
//! - QuoteExecution/RequestExecution require CPI to executor-quoter and executor programs

use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::Mollusk;
use mollusk_svm_bencher::MolluskComputeUnitBencher;
use solana_sdk::{
    account::AccountSharedData,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    system_program,
};

// Router Program ID
const PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0xda, 0x0f, 0x39, 0x58, 0xba, 0x11, 0x3d, 0xfa, 0x31, 0xe1, 0xda, 0xc7, 0x67, 0xe7, 0x47, 0xce,
    0xc9, 0x03, 0xf4, 0x56, 0x9c, 0x89, 0x97, 0x1f, 0x47, 0x27, 0x2e, 0xb0, 0x7e, 0x3d, 0xd5, 0xf9,
]);

// Executor Program ID
const EXECUTOR_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x09, 0xb9, 0x69, 0x71, 0x58, 0x3b, 0x59, 0x03, 0xe0, 0x28, 0x1d, 0xa9, 0x65, 0x48, 0xd5, 0xd2,
    0x3c, 0x65, 0x1f, 0x7a, 0x9c, 0xcd, 0xe3, 0xea, 0xd5, 0x2b, 0x42, 0xf6, 0xb7, 0xda, 0xc2, 0xd2,
]);

// PDA seeds
const CONFIG_SEED: &[u8] = b"config";

// Instruction discriminators
const IX_INITIALIZE: u8 = 0;

// Wormhole chain ID for Solana
const SOLANA_CHAIN_ID: u16 = 1;

fn derive_config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], &PROGRAM_ID)
}

/// Create a funded payer account
fn create_payer_account() -> AccountSharedData {
    AccountSharedData::new(1_000_000_000, 0, &system_program::ID)
}

/// Build Initialize instruction data
fn build_initialize_data(executor_program_id: &Pubkey, our_chain: u16, bump: u8) -> Vec<u8> {
    let mut data = vec![IX_INITIALIZE];
    data.extend_from_slice(executor_program_id.as_ref());
    data.extend_from_slice(&our_chain.to_le_bytes());
    data.push(bump);
    data.push(0); // padding
    data
}

fn main() {
    // Initialize Mollusk with the program
    let mollusk = Mollusk::new(&PROGRAM_ID, "executor_quoter_router");

    // Get the system program keyed account for CPI
    let system_program_account = keyed_account_for_system_program();

    // Set up common accounts
    let payer = Pubkey::new_unique();
    let (config_pda, config_bump) = derive_config_pda();

    // Benchmark: Initialize
    let init_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump),
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    );
    let init_accounts = vec![
        (payer, create_payer_account()),
        (config_pda, AccountSharedData::new(0, 0, &system_program::ID)),
        system_program_account,
    ];

    // Run benchmarks
    // Note: Additional instructions (UpdateQuoterContract, QuoteExecution, RequestExecution)
    // are not benchmarked here due to:
    // - UpdateQuoterContract: requires sol_secp256k1_recover syscall
    // - QuoteExecution: requires CPI to executor-quoter program
    // - RequestExecution: requires CPI to both executor-quoter and executor programs
    //
    // For these instructions, use the integration tests with solana-program-test instead.
    MolluskComputeUnitBencher::new(mollusk)
        .bench(("initialize", &init_ix, &init_accounts))
        .must_pass(true)
        .out_dir("target/benches")
        .execute();
}

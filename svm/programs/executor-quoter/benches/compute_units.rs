//! Compute unit benchmarks for executor-quoter using mollusk-svm.
//!
//! Run with: cargo bench
//! Output: target/benches/executor_quoter_compute_units.md

use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::Mollusk;
use mollusk_svm_bencher::MolluskComputeUnitBencher;
use solana_sdk::{
    account::AccountSharedData,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    rent::Rent,
    system_program,
};

// Program ID - must match the deployed program
const PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x58, 0xce, 0x85, 0x6b, 0x53, 0xca, 0x8b, 0x7d, 0xc9, 0xa3, 0x84, 0x42, 0x1c, 0x5c, 0xaf, 0x30,
    0x63, 0xcf, 0x30, 0x96, 0x2b, 0x4c, 0xf6, 0x0d, 0xad, 0x51, 0x9d, 0x3d, 0xcd, 0xf3, 0x86, 0x58,
]);

// Account discriminators
const CONFIG_DISCRIMINATOR: u8 = 1;
const QUOTE_BODY_DISCRIMINATOR: u8 = 2;
const CHAIN_INFO_DISCRIMINATOR: u8 = 3;

// PDA seeds
const CONFIG_SEED: &[u8] = b"config";
const QUOTE_SEED: &[u8] = b"quote";
const CHAIN_INFO_SEED: &[u8] = b"chain_info";

// Account sizes
const CONFIG_SIZE: usize = 104; // 1 + 1 + 1 + 5 + 32 + 32 + 32
const QUOTE_BODY_SIZE: usize = 40; // 1 + 3 + 2 + 1 + 1 + 8 + 8 + 8 + 8
const CHAIN_INFO_SIZE: usize = 8; // 1 + 1 + 2 + 1 + 1 + 1 + 1

// Instruction discriminators
const IX_INITIALIZE: u8 = 0;
const IX_UPDATE_CHAIN_INFO: u8 = 1;
const IX_UPDATE_QUOTE: u8 = 2;
const IX_REQUEST_QUOTE: u8 = 3;
const IX_REQUEST_EXECUTION_QUOTE: u8 = 4;

// Test chain ID (Ethereum mainnet in Wormhole)
const DST_CHAIN_ID: u16 = 2;

fn derive_config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], &PROGRAM_ID)
}

fn derive_chain_info_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[CHAIN_INFO_SEED, &chain_id.to_le_bytes()],
        &PROGRAM_ID,
    )
}

fn derive_quote_body_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[QUOTE_SEED, &chain_id.to_le_bytes()], &PROGRAM_ID)
}

/// Create a funded payer account
fn create_payer_account() -> AccountSharedData {
    AccountSharedData::new(1_000_000_000, 0, &system_program::ID)
}

/// Create a signer account (updater)
fn create_signer_account() -> AccountSharedData {
    AccountSharedData::new(0, 0, &system_program::ID)
}

/// Create an initialized Config account
fn create_config_account(bump: u8, updater: &Pubkey) -> AccountSharedData {
    let rent = Rent::default();
    let lamports = rent.minimum_balance(CONFIG_SIZE);
    let mut data = vec![0u8; CONFIG_SIZE];

    // Discriminator
    data[0] = CONFIG_DISCRIMINATOR;
    // Bump
    data[1] = bump;
    // src_token_decimals (9 for SOL)
    data[2] = 9;
    // Padding [3..8]
    // quoter_address [8..40]
    data[8..40].copy_from_slice(&[0u8; 32]);
    // updater_address [40..72]
    data[40..72].copy_from_slice(updater.as_ref());
    // payee_address [72..104]
    data[72..104].copy_from_slice(&[1u8; 32]);

    let mut account = AccountSharedData::new(lamports, CONFIG_SIZE, &PROGRAM_ID);
    account.set_data_from_slice(&data);
    account
}

/// Create an initialized ChainInfo account
fn create_chain_info_account(chain_id: u16, bump: u8) -> AccountSharedData {
    let rent = Rent::default();
    let lamports = rent.minimum_balance(CHAIN_INFO_SIZE);
    let mut data = vec![0u8; CHAIN_INFO_SIZE];

    data[0] = CHAIN_INFO_DISCRIMINATOR;
    data[1] = 1; // enabled
    data[2..4].copy_from_slice(&chain_id.to_le_bytes());
    // Old test values: gas_price_decimals=15, native_decimals=18
    data[4] = 15; // gas_price_decimals
    data[5] = 18; // native_decimals (ETH)
    data[6] = bump;

    let mut account = AccountSharedData::new(lamports, CHAIN_INFO_SIZE, &PROGRAM_ID);
    account.set_data_from_slice(&data);
    account
}

/// Create an initialized QuoteBody account
fn create_quote_body_account(chain_id: u16, bump: u8) -> AccountSharedData {
    let rent = Rent::default();
    let lamports = rent.minimum_balance(QUOTE_BODY_SIZE);
    let mut data = vec![0u8; QUOTE_BODY_SIZE];

    data[0] = QUOTE_BODY_DISCRIMINATOR;
    // padding [1..4]
    data[4..6].copy_from_slice(&chain_id.to_le_bytes());
    data[6] = bump;
    // reserved [7]
    // Old test values from executor-quoter-comparison.ts:
    // dst_price (u64 at offset 8) - $16 ETH (test value) in 10^10
    data[8..16].copy_from_slice(&160_000_000u64.to_le_bytes());
    // src_price (u64 at offset 16) - $265 SOL in 10^10
    data[16..24].copy_from_slice(&2_650_000_000u64.to_le_bytes());
    // dst_gas_price (u64 at offset 24) - old test value
    data[24..32].copy_from_slice(&399_146u64.to_le_bytes());
    // base_fee (u64 at offset 32) - old test value
    data[32..40].copy_from_slice(&100u64.to_le_bytes());

    let mut account = AccountSharedData::new(lamports, QUOTE_BODY_SIZE, &PROGRAM_ID);
    account.set_data_from_slice(&data);
    account
}

/// Build Initialize instruction data
fn build_initialize_data(quoter: &Pubkey, updater: &Pubkey, bump: u8) -> Vec<u8> {
    let mut data = vec![IX_INITIALIZE];
    // quoter_address (32 bytes)
    data.extend_from_slice(quoter.as_ref());
    // updater_address (32 bytes)
    data.extend_from_slice(updater.as_ref());
    // src_token_decimals (1 byte)
    data.push(9);
    // bump (1 byte)
    data.push(bump);
    // payee_address (32 bytes)
    data.extend_from_slice(&[1u8; 32]);
    data
}

/// Build UpdateChainInfo instruction data
fn build_update_chain_info_data(chain_id: u16, bump: u8) -> Vec<u8> {
    let mut data = vec![IX_UPDATE_CHAIN_INFO];
    data.extend_from_slice(&chain_id.to_le_bytes());
    data.push(1); // enabled
    data.push(18); // gas_price_decimals
    data.push(18); // native_decimals
    data.push(bump);
    data
}

/// Build UpdateQuote instruction data
fn build_update_quote_data(chain_id: u16, bump: u8) -> Vec<u8> {
    let mut data = vec![IX_UPDATE_QUOTE];
    data.extend_from_slice(&chain_id.to_le_bytes());
    data.push(bump);
    data.extend_from_slice(&[0u8; 5]); // padding
    data.extend_from_slice(&20_000_000_000_000u64.to_le_bytes()); // dst_price
    data.extend_from_slice(&1_500_000_000_000u64.to_le_bytes()); // src_price
    data.extend_from_slice(&30_000_000_000u64.to_le_bytes()); // dst_gas_price
    data.extend_from_slice(&1_000_000u64.to_le_bytes()); // base_fee
    data
}

/// Build RequestQuote instruction data
fn build_request_quote_data(chain_id: u16, gas_limit: u128, msg_value: u128) -> Vec<u8> {
    let mut data = vec![IX_REQUEST_QUOTE];
    // dst_chain (2 bytes)
    data.extend_from_slice(&chain_id.to_le_bytes());
    // dst_addr (32 bytes)
    data.extend_from_slice(&[0xab; 32]);
    // refund_addr (32 bytes)
    data.extend_from_slice(&[0xcd; 32]);
    // request_bytes_len (4 bytes) + request_bytes
    data.extend_from_slice(&0u32.to_le_bytes());
    // relay_instructions_len (4 bytes)
    let relay_len = 33u32; // 1 byte type + 16 bytes gas + 16 bytes value
    data.extend_from_slice(&relay_len.to_le_bytes());
    // relay_instructions: type 1 (Gas)
    data.push(1); // IX_TYPE_GAS
    data.extend_from_slice(&gas_limit.to_be_bytes());
    data.extend_from_slice(&msg_value.to_be_bytes());
    data
}

/// Build RequestExecutionQuote instruction data
fn build_request_execution_quote_data(chain_id: u16, gas_limit: u128, msg_value: u128) -> Vec<u8> {
    let mut data = vec![IX_REQUEST_EXECUTION_QUOTE];
    // dst_chain (2 bytes)
    data.extend_from_slice(&chain_id.to_le_bytes());
    // dst_addr (32 bytes)
    data.extend_from_slice(&[0xab; 32]);
    // refund_addr (32 bytes)
    data.extend_from_slice(&[0xcd; 32]);
    // request_bytes_len (4 bytes) + request_bytes
    data.extend_from_slice(&0u32.to_le_bytes());
    // relay_instructions_len (4 bytes)
    let relay_len = 33u32;
    data.extend_from_slice(&relay_len.to_le_bytes());
    // relay_instructions: type 1 (Gas)
    data.push(1);
    data.extend_from_slice(&gas_limit.to_be_bytes());
    data.extend_from_slice(&msg_value.to_be_bytes());
    data
}

fn main() {
    // Initialize Mollusk with the program
    let mollusk = Mollusk::new(&PROGRAM_ID, "executor_quoter");

    // Get the system program keyed account for CPI
    let system_program_account = keyed_account_for_system_program();

    // Set up common accounts
    let payer = Pubkey::new_unique();
    let updater = Pubkey::new_unique();
    let (config_pda, config_bump) = derive_config_pda();
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(DST_CHAIN_ID);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(DST_CHAIN_ID);

    // Benchmark: Initialize
    let init_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &build_initialize_data(&Pubkey::new_unique(), &updater, config_bump),
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    );
    let init_accounts = vec![
        (payer, create_payer_account()),
        (config_pda, AccountSharedData::new(0, 0, &system_program::ID)),
        system_program_account.clone(),
    ];

    // Benchmark: UpdateChainInfo (create new)
    let update_chain_info_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &build_update_chain_info_data(DST_CHAIN_ID, chain_info_bump),
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(updater, true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(chain_info_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    );
    let update_chain_info_accounts = vec![
        (payer, create_payer_account()),
        (updater, create_signer_account()),
        (config_pda, create_config_account(config_bump, &updater)),
        (chain_info_pda, AccountSharedData::new(0, 0, &system_program::ID)),
        system_program_account.clone(),
    ];

    // Benchmark: UpdateChainInfo (update existing)
    let update_chain_info_existing_accounts = vec![
        (payer, create_payer_account()),
        (updater, create_signer_account()),
        (config_pda, create_config_account(config_bump, &updater)),
        (chain_info_pda, create_chain_info_account(DST_CHAIN_ID, chain_info_bump)),
        system_program_account.clone(),
    ];

    // Benchmark: UpdateQuote (create new)
    let update_quote_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &build_update_quote_data(DST_CHAIN_ID, quote_body_bump),
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(updater, true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quote_body_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    );
    let update_quote_accounts = vec![
        (payer, create_payer_account()),
        (updater, create_signer_account()),
        (config_pda, create_config_account(config_bump, &updater)),
        (quote_body_pda, AccountSharedData::new(0, 0, &system_program::ID)),
        system_program_account.clone(),
    ];

    // Benchmark: UpdateQuote (update existing)
    let update_quote_existing_accounts = vec![
        (payer, create_payer_account()),
        (updater, create_signer_account()),
        (config_pda, create_config_account(config_bump, &updater)),
        (quote_body_pda, create_quote_body_account(DST_CHAIN_ID, quote_body_bump)),
        system_program_account.clone(),
    ];

    // Benchmark: RequestQuote (250k gas, no value) - matches old test
    let request_quote_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &build_request_quote_data(DST_CHAIN_ID, 250_000, 0),
        vec![
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );
    let request_quote_accounts = vec![
        (config_pda, create_config_account(config_bump, &updater)),
        (chain_info_pda, create_chain_info_account(DST_CHAIN_ID, chain_info_bump)),
        (quote_body_pda, create_quote_body_account(DST_CHAIN_ID, quote_body_bump)),
    ];

    // Benchmark: RequestQuote (500k gas, with value)
    let request_quote_large_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &build_request_quote_data(DST_CHAIN_ID, 500_000, 1_000_000_000_000_000_000), // 1 ETH
        vec![
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    // Benchmark: RequestExecutionQuote (250k gas, no value) - matches old test
    let event_cpi = Pubkey::new_unique();
    let request_exec_quote_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &build_request_execution_quote_data(DST_CHAIN_ID, 250_000, 0),
        vec![
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
            AccountMeta::new_readonly(event_cpi, false),
        ],
    );
    let request_exec_quote_accounts = vec![
        (config_pda, create_config_account(config_bump, &updater)),
        (chain_info_pda, create_chain_info_account(DST_CHAIN_ID, chain_info_bump)),
        (quote_body_pda, create_quote_body_account(DST_CHAIN_ID, quote_body_bump)),
        (event_cpi, AccountSharedData::new(0, 0, &system_program::ID)),
    ];

    // Run benchmarks
    MolluskComputeUnitBencher::new(mollusk)
        .bench(("initialize", &init_ix, &init_accounts))
        .bench(("update_chain_info_create", &update_chain_info_ix, &update_chain_info_accounts))
        .bench(("update_chain_info_update", &update_chain_info_ix, &update_chain_info_existing_accounts))
        .bench(("update_quote_create", &update_quote_ix, &update_quote_accounts))
        .bench(("update_quote_update", &update_quote_ix, &update_quote_existing_accounts))
        .bench(("request_quote_250k_gas", &request_quote_ix, &request_quote_accounts))
        .bench(("request_quote_500k_gas_1eth", &request_quote_large_ix, &request_quote_accounts))
        .bench(("request_execution_quote", &request_exec_quote_ix, &request_exec_quote_accounts))
        .must_pass(true)
        .out_dir("target/benches")
        .execute();
}

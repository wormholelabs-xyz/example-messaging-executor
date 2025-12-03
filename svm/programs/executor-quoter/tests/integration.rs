//! Integration tests for executor-quoter using solana-program-test with BPF.
//!
//! Since this is a Pinocchio program (not using solana_program), we must
//! test using the compiled BPF binary rather than native execution.

use solana_program_test::{tokio, ProgramTest};
use solana_sdk::{
    account::Account,
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};

/// Program ID matching the deployed address from Anchor.toml
/// 6yfXVhNgRKRk7YHFT8nTkVpFn5zXktbJddPUWK7jFAGX
const PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x58, 0xce, 0x85, 0x6b, 0x53, 0xca, 0x8b, 0x7d,
    0xc9, 0xa3, 0x84, 0x42, 0x1c, 0x5c, 0xaf, 0x30,
    0x63, 0xcf, 0x30, 0x96, 0x2b, 0x4c, 0xf6, 0x0d,
    0xad, 0x51, 0x9d, 0x3d, 0xcd, 0xf3, 0x86, 0x58,
]);

/// Account discriminators
const CONFIG_DISCRIMINATOR: u8 = 1;
const QUOTE_BODY_DISCRIMINATOR: u8 = 2;
const CHAIN_INFO_DISCRIMINATOR: u8 = 3;

/// PDA seeds
const CONFIG_SEED: &[u8] = b"config";
const QUOTE_SEED: &[u8] = b"quote";
const CHAIN_INFO_SEED: &[u8] = b"chain_info";

/// Account sizes
const CONFIG_SIZE: usize = 104;
const CHAIN_INFO_SIZE: usize = 8;
const QUOTE_BODY_SIZE: usize = 40;

/// Instruction discriminators
const IX_INITIALIZE: u8 = 0;
const IX_UPDATE_CHAIN_INFO: u8 = 1;
const IX_UPDATE_QUOTE: u8 = 2;
const IX_REQUEST_QUOTE: u8 = 3;
const IX_REQUEST_EXECUTION_QUOTE: u8 = 4;

/// Helper to derive config PDA
fn derive_config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], &PROGRAM_ID)
}

/// Helper to derive chain_info PDA
fn derive_chain_info_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CHAIN_INFO_SEED, &chain_id.to_le_bytes()], &PROGRAM_ID)
}

/// Helper to derive quote_body PDA
fn derive_quote_body_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[QUOTE_SEED, &chain_id.to_le_bytes()], &PROGRAM_ID)
}

/// Build Initialize instruction data
fn build_initialize_data(
    quoter_address: &Pubkey,
    updater_address: &Pubkey,
    src_token_decimals: u8,
    bump: u8,
    payee_address: &[u8; 32],
) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 32 + 32 + 1 + 1 + 30 + 32);
    data.push(IX_INITIALIZE);
    data.extend_from_slice(quoter_address.as_ref());
    data.extend_from_slice(updater_address.as_ref());
    data.push(src_token_decimals);
    data.push(bump);
    data.extend_from_slice(&[0u8; 30]); // padding (reduced by 1 for bump)
    data.extend_from_slice(payee_address);
    data
}

/// Build UpdateChainInfo instruction data
fn build_update_chain_info_data(
    chain_id: u16,
    enabled: bool,
    gas_price_decimals: u8,
    native_decimals: u8,
    bump: u8,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 8);
    data.push(IX_UPDATE_CHAIN_INFO);
    data.extend_from_slice(&chain_id.to_le_bytes());
    data.push(if enabled { 1 } else { 0 });
    data.push(gas_price_decimals);
    data.push(native_decimals);
    data.push(bump);
    data.extend_from_slice(&[0u8; 2]); // padding (reduced by 1 for bump)
    data
}

/// Build UpdateQuote instruction data
fn build_update_quote_data(
    chain_id: u16,
    bump: u8,
    dst_price: u64,
    src_price: u64,
    dst_gas_price: u64,
    base_fee: u64,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 40);
    data.push(IX_UPDATE_QUOTE);
    data.extend_from_slice(&chain_id.to_le_bytes());
    data.push(bump);
    data.extend_from_slice(&[0u8; 5]); // padding (reduced by 1 for bump)
    data.extend_from_slice(&dst_price.to_le_bytes());
    data.extend_from_slice(&src_price.to_le_bytes());
    data.extend_from_slice(&dst_gas_price.to_le_bytes());
    data.extend_from_slice(&base_fee.to_le_bytes());
    data
}

/// Build RequestQuote instruction data
fn build_request_quote_data(
    dst_chain: u16,
    dst_addr: &[u8; 32],
    refund_addr: &[u8; 32],
    request_bytes: &[u8],
    relay_instructions: &[u8],
) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(IX_REQUEST_QUOTE);
    data.extend_from_slice(&dst_chain.to_le_bytes());
    data.extend_from_slice(dst_addr);
    data.extend_from_slice(refund_addr);
    data.extend_from_slice(&(request_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(request_bytes);
    data.extend_from_slice(&(relay_instructions.len() as u32).to_le_bytes());
    data.extend_from_slice(relay_instructions);
    data
}

/// Build RequestExecutionQuote instruction data
fn build_request_execution_quote_data(
    dst_chain: u16,
    dst_addr: &[u8; 32],
    refund_addr: &[u8; 32],
    request_bytes: &[u8],
    relay_instructions: &[u8],
) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(IX_REQUEST_EXECUTION_QUOTE);
    data.extend_from_slice(&dst_chain.to_le_bytes());
    data.extend_from_slice(dst_addr);
    data.extend_from_slice(refund_addr);
    data.extend_from_slice(&(request_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(request_bytes);
    data.extend_from_slice(&(relay_instructions.len() as u32).to_le_bytes());
    data.extend_from_slice(relay_instructions);
    data
}

/// Build relay instructions with gas limit and msg value
fn build_relay_instructions_gas(gas_limit: u128, msg_value: u128) -> Vec<u8> {
    let mut data = Vec::with_capacity(33);
    data.push(1); // IX_TYPE_GAS
    data.extend_from_slice(&gas_limit.to_be_bytes());
    data.extend_from_slice(&msg_value.to_be_bytes());
    data
}

/// Create a Config account with initialized data
fn create_config_account_data(
    bump: u8,
    src_token_decimals: u8,
    quoter_address: &Pubkey,
    updater_address: &Pubkey,
    payee_address: &[u8; 32],
) -> Vec<u8> {
    let mut data = vec![0u8; CONFIG_SIZE];
    data[0] = CONFIG_DISCRIMINATOR;
    data[1] = bump;
    data[2] = src_token_decimals;
    // padding at 3..8
    data[8..40].copy_from_slice(quoter_address.as_ref());
    data[40..72].copy_from_slice(updater_address.as_ref());
    data[72..104].copy_from_slice(payee_address);
    data
}

/// Create a ChainInfo account with initialized data
fn create_chain_info_account_data(
    bump: u8,
    chain_id: u16,
    enabled: bool,
    gas_price_decimals: u8,
    native_decimals: u8,
) -> Vec<u8> {
    let mut data = vec![0u8; CHAIN_INFO_SIZE];
    data[0] = CHAIN_INFO_DISCRIMINATOR;
    data[1] = if enabled { 1 } else { 0 };
    data[2..4].copy_from_slice(&chain_id.to_le_bytes());
    data[4] = gas_price_decimals;
    data[5] = native_decimals;
    data[6] = bump;
    data[7] = 0; // reserved
    data
}

/// Create a QuoteBody account with initialized data
fn create_quote_body_account_data(
    bump: u8,
    chain_id: u16,
    dst_price: u64,
    src_price: u64,
    dst_gas_price: u64,
    base_fee: u64,
) -> Vec<u8> {
    let mut data = vec![0u8; QUOTE_BODY_SIZE];
    data[0] = QUOTE_BODY_DISCRIMINATOR;
    // padding at 1..4
    data[4..6].copy_from_slice(&chain_id.to_le_bytes());
    data[6] = bump;
    data[7] = 0; // reserved
    data[8..16].copy_from_slice(&dst_price.to_le_bytes());
    data[16..24].copy_from_slice(&src_price.to_le_bytes());
    data[24..32].copy_from_slice(&dst_gas_price.to_le_bytes());
    data[32..40].copy_from_slice(&base_fee.to_le_bytes());
    data
}

/// Create a ProgramTest loading the BPF program binary
fn create_program_test() -> ProgramTest {
    let mut pt = ProgramTest::default();
    // Load the BPF program directly from the target/deploy directory
    pt.add_program("executor_quoter", PROGRAM_ID, None);
    // Force BPF execution for pinocchio programs
    pt.prefer_bpf(true);
    pt
}

#[tokio::test]
async fn test_initialize() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let (config_pda, config_bump) = derive_config_pda();
    let quoter_address = Pubkey::new_unique();
    let updater_address = Pubkey::new_unique();
    let payee_address = [0x42u8; 32];

    // Add payer with funds
    pt.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let instruction_data = build_initialize_data(
        &quoter_address,
        &updater_address,
        9, // SOL decimals
        config_bump,
        &payee_address,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Initialize failed: {:?}", result.err());

    // Verify config account was created
    let config_account = banks_client
        .get_account(config_pda)
        .await
        .expect("Failed to get account")
        .expect("Config account not found");

    assert_eq!(config_account.data.len(), CONFIG_SIZE);
    assert_eq!(config_account.data[0], CONFIG_DISCRIMINATOR);
    assert_eq!(config_account.data[2], 9); // src_token_decimals

    println!("Initialize test passed!");
}

#[tokio::test]
async fn test_update_chain_info() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = Keypair::new();
    let (config_pda, config_bump) = derive_config_pda();
    let chain_id: u16 = 2; // Ethereum
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let payee_address = [0x42u8; 32];

    // Add payer and updater with funds
    pt.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    pt.add_account(
        updater.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Add pre-existing config account
    let config_data = create_config_account_data(
        config_bump,
        9,
        &Pubkey::new_unique(),
        &updater.pubkey(),
        &payee_address,
    );
    pt.add_account(
        config_pda,
        Account {
            lamports: 1_000_000_000,
            data: config_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let instruction_data = build_update_chain_info_data(
        chain_id,
        true, // enabled
        9,    // gas_price_decimals (Gwei)
        18,   // native_decimals (ETH)
        chain_info_bump,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(chain_info_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer, &updater], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "UpdateChainInfo failed: {:?}",
        result.err()
    );

    // Verify chain_info account
    let chain_info_account = banks_client
        .get_account(chain_info_pda)
        .await
        .expect("Failed to get account")
        .expect("ChainInfo account not found");

    assert_eq!(chain_info_account.data.len(), CHAIN_INFO_SIZE);
    assert_eq!(chain_info_account.data[0], CHAIN_INFO_DISCRIMINATOR);
    assert_eq!(chain_info_account.data[1], 1); // enabled

    println!("UpdateChainInfo test passed!");
}

#[tokio::test]
async fn test_update_quote() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = Keypair::new();
    let (config_pda, config_bump) = derive_config_pda();
    let chain_id: u16 = 2;
    let (quote_body_pda, quote_bump) = derive_quote_body_pda(chain_id);
    let payee_address = [0x42u8; 32];

    // Add payer and updater
    pt.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    pt.add_account(
        updater.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Add pre-existing config account
    let config_data = create_config_account_data(
        config_bump,
        9,
        &Pubkey::new_unique(),
        &updater.pubkey(),
        &payee_address,
    );
    pt.add_account(
        config_pda,
        Account {
            lamports: 1_000_000_000,
            data: config_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let instruction_data = build_update_quote_data(
        chain_id,
        quote_bump,
        2000_0000000000, // dst_price: $2000 in 10^10
        200_0000000000,  // src_price: $200 in 10^10
        50_000000000,    // dst_gas_price: 50 Gwei
        1000000,         // base_fee: 0.001 SOL
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quote_body_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer, &updater], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "UpdateQuote failed: {:?}", result.err());

    // Verify quote_body account
    let quote_body_account = banks_client
        .get_account(quote_body_pda)
        .await
        .expect("Failed to get account")
        .expect("QuoteBody account not found");

    assert_eq!(quote_body_account.data.len(), QUOTE_BODY_SIZE);
    assert_eq!(quote_body_account.data[0], QUOTE_BODY_DISCRIMINATOR);

    println!("UpdateQuote test passed!");
}

#[tokio::test]
async fn test_request_quote() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = Keypair::new();
    let (config_pda, config_bump) = derive_config_pda();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let payee_address = [0x42u8; 32];

    // Add payer
    pt.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Add pre-existing accounts
    let config_data = create_config_account_data(
        config_bump,
        9,
        &Pubkey::new_unique(),
        &updater.pubkey(),
        &payee_address,
    );
    pt.add_account(
        config_pda,
        Account {
            lamports: 1_000_000_000,
            data: config_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true, // enabled
        9,    // gas_price_decimals
        18,   // native_decimals
    );
    pt.add_account(
        chain_info_pda,
        Account {
            lamports: 1_000_000_000,
            data: chain_info_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let quote_body_data = create_quote_body_account_data(
        quote_body_bump,
        chain_id,
        2000_0000000000, // dst_price
        200_0000000000,  // src_price
        50_000000000,    // dst_gas_price (50 Gwei)
        1000000,         // base_fee
    );
    pt.add_account(
        quote_body_pda,
        Account {
            lamports: 1_000_000_000,
            data: quote_body_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let dst_addr = [0x01u8; 32];
    let refund_addr = [0x02u8; 32];
    let relay_instructions = build_relay_instructions_gas(200000, 0); // 200k gas, 0 msg value

    let instruction_data =
        build_request_quote_data(chain_id, &dst_addr, &refund_addr, &[], &relay_instructions);

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "RequestQuote failed: {:?}", result.err());

    println!("RequestQuote test passed!");
}

#[tokio::test]
async fn test_request_execution_quote() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = Keypair::new();
    let (config_pda, config_bump) = derive_config_pda();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let payee_address = [0x42u8; 32];

    // Add payer
    pt.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Add pre-existing accounts
    let config_data = create_config_account_data(
        config_bump,
        9,
        &Pubkey::new_unique(),
        &updater.pubkey(),
        &payee_address,
    );
    pt.add_account(
        config_pda,
        Account {
            lamports: 1_000_000_000,
            data: config_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true, // enabled
        9,    // gas_price_decimals
        18,   // native_decimals
    );
    pt.add_account(
        chain_info_pda,
        Account {
            lamports: 1_000_000_000,
            data: chain_info_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let quote_body_data = create_quote_body_account_data(
        quote_body_bump,
        chain_id,
        2000_0000000000, // dst_price
        200_0000000000,  // src_price
        50_000000000,    // dst_gas_price
        1000000,         // base_fee
    );
    pt.add_account(
        quote_body_pda,
        Account {
            lamports: 1_000_000_000,
            data: quote_body_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let dst_addr = [0x01u8; 32];
    let refund_addr = [0x02u8; 32];
    let relay_instructions = build_relay_instructions_gas(200000, 0);

    let instruction_data = build_request_execution_quote_data(
        chain_id,
        &dst_addr,
        &refund_addr,
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_ok(),
        "RequestExecutionQuote failed: {:?}",
        result.err()
    );

    println!("RequestExecutionQuote test passed!");
}

#[tokio::test]
async fn test_invalid_updater() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let authorized_updater = Keypair::new();
    let unauthorized_updater = Keypair::new();
    let (config_pda, config_bump) = derive_config_pda();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let payee_address = [0x42u8; 32];

    // Add payer and unauthorized updater
    pt.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    pt.add_account(
        unauthorized_updater.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    // Create config with authorized_updater
    let config_data = create_config_account_data(
        config_bump,
        9,
        &Pubkey::new_unique(),
        &authorized_updater.pubkey(), // Note: this is the authorized one
        &payee_address,
    );
    pt.add_account(
        config_pda,
        Account {
            lamports: 1_000_000_000,
            data: config_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let instruction_data = build_update_chain_info_data(chain_id, true, 9, 18, chain_info_bump);

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(unauthorized_updater.pubkey(), true), // Using unauthorized
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(chain_info_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer, &unauthorized_updater], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(
        result.is_err(),
        "Should have failed with InvalidUpdater error"
    );

    println!("InvalidUpdater test passed!");
}

#[tokio::test]
async fn test_chain_disabled() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = Keypair::new();
    let (config_pda, config_bump) = derive_config_pda();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let payee_address = [0x42u8; 32];

    // Add payer
    pt.add_account(
        payer.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let config_data = create_config_account_data(
        config_bump,
        9,
        &Pubkey::new_unique(),
        &updater.pubkey(),
        &payee_address,
    );
    pt.add_account(
        config_pda,
        Account {
            lamports: 1_000_000_000,
            data: config_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    // Create chain_info with enabled = false
    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        false, // DISABLED
        9,
        18,
    );
    pt.add_account(
        chain_info_pda,
        Account {
            lamports: 1_000_000_000,
            data: chain_info_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let quote_body_data = create_quote_body_account_data(
        quote_body_bump,
        chain_id,
        2000_0000000000,
        200_0000000000,
        50_000000000,
        1000000,
    );
    pt.add_account(
        quote_body_pda,
        Account {
            lamports: 1_000_000_000,
            data: quote_body_data,
            owner: PROGRAM_ID,
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let dst_addr = [0x01u8; 32];
    let refund_addr = [0x02u8; 32];
    let relay_instructions = build_relay_instructions_gas(200000, 0);

    let instruction_data =
        build_request_quote_data(chain_id, &dst_addr, &refund_addr, &[], &relay_instructions);

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with ChainDisabled");

    println!("ChainDisabled test passed!");
}

#[tokio::test]
async fn test_full_flow() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = Keypair::new();
    let quoter = Pubkey::new_unique();
    let (config_pda, config_bump) = derive_config_pda();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_bump) = derive_quote_body_pda(chain_id);
    let payee_address = [0x42u8; 32];

    // Add payer and updater with funds
    pt.add_account(
        payer.pubkey(),
        Account {
            lamports: 10_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );
    pt.add_account(
        updater.pubkey(),
        Account {
            lamports: 1_000_000_000,
            data: vec![],
            owner: system_program::id(),
            executable: false,
            rent_epoch: 0,
        },
    );

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    // Step 1: Initialize
    let init_data = build_initialize_data(&quoter, &updater.pubkey(), 9, config_bump, &payee_address);
    let init_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &init_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut tx = Transaction::new_with_payer(&[init_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], recent_blockhash);
    banks_client
        .process_transaction(tx)
        .await
        .expect("Initialize failed");
    println!("Step 1: Initialize - PASSED");

    // Step 2: UpdateChainInfo
    let update_chain_data = build_update_chain_info_data(chain_id, true, 9, 18, chain_info_bump);
    let update_chain_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &update_chain_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(chain_info_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    // Get fresh blockhash for each transaction
    let recent_blockhash = banks_client
        .get_latest_blockhash()
        .await
        .expect("get blockhash");
    // Add compute budget instruction to allow more CUs
    let compute_budget_ix = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
    let mut tx = Transaction::new_with_payer(&[compute_budget_ix, update_chain_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer, &updater], recent_blockhash);
    banks_client
        .process_transaction(tx)
        .await
        .expect("UpdateChainInfo failed");
    println!("Step 2: UpdateChainInfo - PASSED");

    // Step 3: UpdateQuote
    let update_quote_data =
        build_update_quote_data(chain_id, quote_bump, 2000_0000000000, 200_0000000000, 50_000000000, 1000000);
    let update_quote_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &update_quote_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quote_body_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let recent_blockhash = banks_client
        .get_latest_blockhash()
        .await
        .expect("get blockhash");
    let mut tx = Transaction::new_with_payer(&[update_quote_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer, &updater], recent_blockhash);
    banks_client
        .process_transaction(tx)
        .await
        .expect("UpdateQuote failed");
    println!("Step 3: UpdateQuote - PASSED");

    // Step 4: RequestQuote
    let relay_instructions = build_relay_instructions_gas(200000, 0);
    let request_quote_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );
    let request_quote_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &request_quote_data,
        vec![
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let recent_blockhash = banks_client
        .get_latest_blockhash()
        .await
        .expect("get blockhash");
    let mut tx = Transaction::new_with_payer(&[request_quote_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], recent_blockhash);
    banks_client
        .process_transaction(tx)
        .await
        .expect("RequestQuote failed");
    println!("Step 4: RequestQuote - PASSED");

    println!("\nFull flow completed successfully!");
}

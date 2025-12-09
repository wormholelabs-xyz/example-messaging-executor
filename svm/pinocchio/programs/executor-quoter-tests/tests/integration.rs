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

/// Account discriminators (updated - no config)
const QUOTE_BODY_DISCRIMINATOR: u8 = 1;
const CHAIN_INFO_DISCRIMINATOR: u8 = 2;

/// PDA seeds
const QUOTE_SEED: &[u8] = b"quote";
const CHAIN_INFO_SEED: &[u8] = b"chain_info";

/// Account sizes
const CHAIN_INFO_SIZE: usize = 8;
const QUOTE_BODY_SIZE: usize = 40;

/// Instruction discriminators (8 bytes, Anchor-compatible)
/// Byte 0 = instruction ID, bytes 1-7 = padding (zeros)
const IX_UPDATE_CHAIN_INFO: [u8; 8] = [0, 0, 0, 0, 0, 0, 0, 0];
const IX_UPDATE_QUOTE: [u8; 8] = [1, 0, 0, 0, 0, 0, 0, 0];
const IX_REQUEST_QUOTE: [u8; 8] = [2, 0, 0, 0, 0, 0, 0, 0];
const IX_REQUEST_EXECUTION_QUOTE: [u8; 8] = [3, 0, 0, 0, 0, 0, 0, 0];


/// Get the authorized updater keypair.
/// Reads from QUOTER_UPDATER_KEYPAIR_PATH env var (path to JSON keypair file).
/// The program must be built with QUOTER_UPDATER_PUBKEY set to this keypair's pubkey.
fn get_updater_keypair() -> Keypair {
    let keypair_path = std::env::var("QUOTER_UPDATER_KEYPAIR_PATH")
        .expect("QUOTER_UPDATER_KEYPAIR_PATH env var must be set to path of updater keypair JSON file");
    solana_sdk::signature::read_keypair_file(&keypair_path)
        .expect("Failed to read updater keypair from file")
}

/// Get the payee address (32 bytes) from the updater keypair.
/// The program must be built with QUOTER_PAYEE_PUBKEY set to this value.
fn get_payee_address() -> [u8; 32] {
    get_updater_keypair().pubkey().to_bytes()
}

/// Get a dummy config pubkey for the _config account parameter.
/// This account is unused in get_quote instructions but required for the interface.
fn get_dummy_config_pubkey() -> Pubkey {
    Pubkey::new_from_array([0u8; 32])
}

/// Helper to derive chain_info PDA
fn derive_chain_info_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CHAIN_INFO_SEED, &chain_id.to_le_bytes()], &PROGRAM_ID)
}

/// Helper to derive quote_body PDA
fn derive_quote_body_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[QUOTE_SEED, &chain_id.to_le_bytes()], &PROGRAM_ID)
}

/// Build UpdateChainInfo instruction data
fn build_update_chain_info_data(
    chain_id: u16,
    enabled: bool,
    gas_price_decimals: u8,
    native_decimals: u8,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(8 + 6);
    data.extend_from_slice(&IX_UPDATE_CHAIN_INFO); // 8-byte discriminator
    data.extend_from_slice(&chain_id.to_le_bytes());
    data.push(if enabled { 1 } else { 0 });
    data.push(gas_price_decimals);
    data.push(native_decimals);
    data.push(0); // padding (was bump)
    data
}

/// Build UpdateQuote instruction data
fn build_update_quote_data(
    chain_id: u16,
    dst_price: u64,
    src_price: u64,
    dst_gas_price: u64,
    base_fee: u64,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(8 + 40);
    data.extend_from_slice(&IX_UPDATE_QUOTE); // 8-byte discriminator
    data.extend_from_slice(&chain_id.to_le_bytes());
    data.extend_from_slice(&[0u8; 6]); // padding (was bump + 5 padding)
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
    data.extend_from_slice(&IX_REQUEST_QUOTE); // 8-byte discriminator
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
    data.extend_from_slice(&IX_REQUEST_EXECUTION_QUOTE); // 8-byte discriminator
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

/// Build drop-off relay instruction (Type 2)
fn build_relay_instructions_dropoff(msg_value: u128, recipient: &[u8; 32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(49);
    data.push(2); // IX_TYPE_DROP_OFF
    data.extend_from_slice(&msg_value.to_be_bytes());
    data.extend_from_slice(recipient);
    data
}

/// Build combined gas + dropoff relay instructions
fn build_relay_instructions_gas_and_dropoff(
    gas_limit: u128,
    gas_msg_value: u128,
    dropoff_value: u128,
    recipient: &[u8; 32],
) -> Vec<u8> {
    let mut data = Vec::with_capacity(33 + 49);
    // Gas instruction (Type 1)
    data.push(1);
    data.extend_from_slice(&gas_limit.to_be_bytes());
    data.extend_from_slice(&gas_msg_value.to_be_bytes());
    // DropOff instruction (Type 2)
    data.push(2);
    data.extend_from_slice(&dropoff_value.to_be_bytes());
    data.extend_from_slice(recipient);
    data
}

/// Build relay instruction with invalid type
fn build_relay_instructions_invalid_type() -> Vec<u8> {
    let mut data = Vec::with_capacity(33);
    data.push(0xFF); // Invalid type
    data.extend_from_slice(&100u128.to_be_bytes());
    data.extend_from_slice(&0u128.to_be_bytes());
    data
}

/// Build two dropoff instructions (invalid - only one allowed)
fn build_relay_instructions_two_dropoffs(recipient: &[u8; 32]) -> Vec<u8> {
    let mut data = Vec::with_capacity(98);
    // First dropoff
    data.push(2);
    data.extend_from_slice(&100u128.to_be_bytes());
    data.extend_from_slice(recipient);
    // Second dropoff (this is invalid)
    data.push(2);
    data.extend_from_slice(&200u128.to_be_bytes());
    data.extend_from_slice(recipient);
    data
}

/// Build truncated relay instruction (missing bytes)
fn build_relay_instructions_truncated() -> Vec<u8> {
    let mut data = Vec::with_capacity(10);
    data.push(1); // Gas type
    data.extend_from_slice(&[0u8; 8]); // Only 8 bytes instead of 32
    data
}

/// Create a ChainInfo account with initialized data
/// Layout: discriminator, bump, chain_id (u16), enabled, gas_price_decimals, native_decimals, padding
fn create_chain_info_account_data(
    bump: u8,
    chain_id: u16,
    enabled: bool,
    gas_price_decimals: u8,
    native_decimals: u8,
) -> Vec<u8> {
    let mut data = vec![0u8; CHAIN_INFO_SIZE];
    data[0] = CHAIN_INFO_DISCRIMINATOR;
    data[1] = bump;
    data[2..4].copy_from_slice(&chain_id.to_le_bytes());
    data[4] = if enabled { 1 } else { 0 };
    data[5] = gas_price_decimals;
    data[6] = native_decimals;
    data[7] = 0; // padding
    data
}

/// Create a QuoteBody account with initialized data
/// Layout: discriminator, bump, chain_id (u16), padding (4), dst_price, src_price, dst_gas_price, base_fee
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
    data[1] = bump;
    data[2..4].copy_from_slice(&chain_id.to_le_bytes());
    // padding at 4..8
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
async fn test_update_chain_info() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2; // Ethereum
    let (chain_info_pda, _chain_info_bump) = derive_chain_info_pda(chain_id);

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

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let instruction_data = build_update_chain_info_data(
        chain_id,
        true, // enabled
        9,    // gas_price_decimals (Gwei)
        18,   // native_decimals (ETH)
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
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
    // ChainInfo layout: discriminator (0), bump (1), chain_id (2-3), enabled (4)
    assert_eq!(chain_info_account.data[4], 1); // enabled

    println!("UpdateChainInfo test passed!");
}

#[tokio::test]
async fn test_update_quote() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (quote_body_pda, quote_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();

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


    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let instruction_data = build_update_quote_data(
        chain_id,
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
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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
            AccountMeta::new_readonly(config_pubkey, false),
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
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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

    // Use system program as a dummy event_cpi account (required but unused)
    let event_cpi = system_program::ID;

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
            AccountMeta::new_readonly(event_cpi, false),
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
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let _payee_address = get_payee_address();

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

    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let instruction_data = build_update_chain_info_data(chain_id, true, 9, 18);

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(unauthorized_updater.pubkey(), true), // Using unauthorized
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
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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
            AccountMeta::new_readonly(config_pubkey, false),
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
    let updater = get_updater_keypair();
    let quoter = Pubkey::new_unique();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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

    // Step 1: UpdateChainInfo
    let update_chain_data = build_update_chain_info_data(chain_id, true, 9, 18);
    let update_chain_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &update_chain_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
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
        build_update_quote_data(chain_id, 2000_0000000000, 200_0000000000, 50_000000000, 1000000);
    let update_quote_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &update_quote_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
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
            AccountMeta::new_readonly(config_pubkey, false),
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

// ============================================================================
// ERROR PATH TESTS
// ============================================================================

#[tokio::test]
async fn test_invalid_updater_quote() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let authorized_updater = Keypair::new();
    let unauthorized_updater = Keypair::new();
    let chain_id: u16 = 2;
    let (quote_body_pda, quote_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let (mut banks_client, _, recent_blockhash) = pt.start().await;

    let instruction_data = build_update_quote_data(
        chain_id,
        2000_0000000000,
        200_0000000000,
        50_000000000,
        1000000,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(unauthorized_updater.pubkey(), true),
            AccountMeta::new(quote_body_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer, &unauthorized_updater], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with InvalidUpdater");

    println!("InvalidUpdater (UpdateQuote) test passed!");
}

#[tokio::test]
async fn test_chain_disabled_execution_quote() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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

    let relay_instructions = build_relay_instructions_gas(200000, 0);
    let instruction_data = build_request_execution_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    // Use system program as a dummy event_cpi account (required but unused)
    let event_cpi = system_program::ID;

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
            AccountMeta::new_readonly(event_cpi, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with ChainDisabled");

    println!("ChainDisabled (RequestExecutionQuote) test passed!");
}

#[tokio::test]
async fn test_unsupported_instruction() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
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

    let relay_instructions = build_relay_instructions_invalid_type();
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with UnsupportedInstruction");

    println!("UnsupportedInstruction test passed!");
}

#[tokio::test]
async fn test_more_than_one_dropoff() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
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

    let relay_instructions = build_relay_instructions_two_dropoffs(&[0x03u8; 32]);
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with MoreThanOneDropOff");

    println!("MoreThanOneDropOff test passed!");
}

#[tokio::test]
async fn test_invalid_relay_instructions() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
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

    let relay_instructions = build_relay_instructions_truncated();
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with InvalidRelayInstructions");

    println!("InvalidRelayInstructions test passed!");
}

#[tokio::test]
async fn test_not_enough_accounts() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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

    let relay_instructions = build_relay_instructions_gas(200000, 0);
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    // Only provide config account, missing chain_info and quote_body
    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with NotEnoughAccountKeys");

    println!("NotEnoughAccountKeys test passed!");
}

// ============================================================================
// EDGE CASE / BOUNDARY TESTS
// ============================================================================

#[tokio::test]
async fn test_zero_gas_limit() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
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
        1000000, // base_fee = 1000000 (10^6 at QUOTE_DECIMALS=10 = 0.0001)
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

    // Zero gas limit, zero msg value - should return base_fee only
    let relay_instructions = build_relay_instructions_gas(0, 0);
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Zero gas limit should succeed: {:?}", result.err());

    println!("Zero gas limit test passed!");
}

#[tokio::test]
async fn test_zero_src_price() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
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

    // src_price = 0 (division by zero scenario)
    let quote_body_data = create_quote_body_account_data(
        quote_body_bump,
        chain_id,
        2000_0000000000, // dst_price
        0,               // src_price = 0!
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

    let relay_instructions = build_relay_instructions_gas(200000, 0);
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with MathOverflow (division by zero)");

    println!("Zero src_price test passed!");
}

#[tokio::test]
async fn test_multiple_gas_instructions() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
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

    // Build two gas instructions - they should sum
    let mut relay_instructions = build_relay_instructions_gas(100000, 1000000000000000000); // 100k gas, 1 ETH
    relay_instructions.extend(build_relay_instructions_gas(50000, 500000000000000000)); // 50k gas, 0.5 ETH

    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Multiple gas instructions should succeed: {:?}", result.err());

    println!("Multiple gas instructions test passed!");
}

#[tokio::test]
async fn test_gas_plus_dropoff() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
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

    // Gas instruction + DropOff instruction
    let relay_instructions = build_relay_instructions_gas_and_dropoff(
        200000,              // gas_limit
        0,                   // gas msg_value
        1000000000000000000, // dropoff value (1 ETH)
        &[0x03u8; 32],       // recipient
    );

    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Gas + DropOff should succeed: {:?}", result.err());

    println!("Gas + DropOff test passed!");
}

#[tokio::test]
async fn test_empty_relay_instructions() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
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

    // Empty relay instructions - should just return base_fee
    let relay_instructions: Vec<u8> = vec![];
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Empty relay instructions should succeed: {:?}", result.err());

    println!("Empty relay instructions test passed!");
}

#[tokio::test]
async fn test_arithmetic_overflow() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
        0, // gas_price_decimals = 0 (makes values larger)
        0, // native_decimals = 0 (makes values larger)
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

    // Extreme prices to cause overflow
    let quote_body_data = create_quote_body_account_data(
        quote_body_bump,
        chain_id,
        u64::MAX, // dst_price
        1,        // tiny src_price
        u64::MAX, // dst_gas_price
        u64::MAX, // base_fee
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

    // Very large values that should cause overflow
    let relay_instructions = build_relay_instructions_gas(u128::MAX / 2, u128::MAX / 2);
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_err(), "Should have failed with MathOverflow");

    println!("Arithmetic overflow test passed!");
}

// ============================================================================
// DECIMAL NORMALIZATION TESTS
// ============================================================================

#[tokio::test]
async fn test_decimals_18_to_9() {
    // Test with dst_native_decimals=18 (ETH), which tests the division path
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
        9,  // gas_price_decimals (Gwei)
        18, // native_decimals (ETH)
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

    let relay_instructions = build_relay_instructions_gas(200000, 1000000000000000000); // 200k gas, 1 ETH
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Decimals 18->9 should succeed: {:?}", result.err());

    println!("Decimals 18->9 test passed!");
}

#[tokio::test]
async fn test_decimals_6_to_9() {
    // Test with dst_native_decimals=6 (USDC chain), which tests the multiplication path
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
        6, // gas_price_decimals
        6, // native_decimals (6 decimals like USDC)
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
        1_0000000000, // dst_price: $1 at 10^10
        200_0000000000, // src_price: $200 at 10^10
        1_000000,    // dst_gas_price: 1 at 6 decimals
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

    let relay_instructions = build_relay_instructions_gas(200000, 1_000000); // 200k gas, 1 unit at 6 decimals
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Decimals 6->9 should succeed: {:?}", result.err());

    println!("Decimals 6->9 test passed!");
}

#[tokio::test]
async fn test_decimals_9_to_9() {
    // Test with dst_native_decimals=9 (same as SVM), identity path
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true,
        9, // gas_price_decimals (same as SOL)
        9, // native_decimals (same as SOL)
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
        200_0000000000, // dst_price (same as src)
        200_0000000000, // src_price
        1_000000000,    // dst_gas_price at 9 decimals
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

    let relay_instructions = build_relay_instructions_gas(200000, 1_000000000); // 200k gas, 1 SOL
    let instruction_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );

    let instruction = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );

    let mut transaction = Transaction::new_with_payer(&[instruction], Some(&payer.pubkey()));
    transaction.sign(&[&payer], recent_blockhash);

    let result = banks_client.process_transaction(transaction).await;
    assert!(result.is_ok(), "Decimals 9->9 should succeed: {:?}", result.err());

    println!("Decimals 9->9 test passed!");
}

// ============================================================================
// ACCOUNT STATE VERIFICATION TESTS
// ============================================================================

#[tokio::test]
async fn test_update_overwrites_quote() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (quote_body_pda, quote_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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

    // First update
    let instruction_data1 = build_update_quote_data(
        chain_id,
        1000_0000000000, // dst_price
        100_0000000000,  // src_price
        25_000000000,    // dst_gas_price
        500000,          // base_fee
    );

    let instruction1 = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data1,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
            AccountMeta::new(quote_body_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut tx1 = Transaction::new_with_payer(&[instruction1], Some(&payer.pubkey()));
    tx1.sign(&[&payer, &updater], recent_blockhash);
    banks_client.process_transaction(tx1).await.expect("First update failed");

    // Second update with different values
    let recent_blockhash = banks_client.get_latest_blockhash().await.expect("get blockhash");
    let instruction_data2 = build_update_quote_data(
        chain_id,
        2000_0000000000, // different dst_price
        200_0000000000,  // different src_price
        50_000000000,    // different dst_gas_price
        1000000,         // different base_fee
    );

    let instruction2 = Instruction::new_with_bytes(
        PROGRAM_ID,
        &instruction_data2,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
            AccountMeta::new(quote_body_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut tx2 = Transaction::new_with_payer(&[instruction2], Some(&payer.pubkey()));
    tx2.sign(&[&payer, &updater], recent_blockhash);
    banks_client.process_transaction(tx2).await.expect("Second update failed");

    // Verify the new values
    let quote_body_account = banks_client
        .get_account(quote_body_pda)
        .await
        .expect("Failed to get account")
        .expect("QuoteBody account not found");

    // Check dst_price at offset 8 (after discriminator/padding/chain_id/bump/reserved)
    let dst_price = u64::from_le_bytes(quote_body_account.data[8..16].try_into().unwrap());
    assert_eq!(dst_price, 2000_0000000000, "dst_price should be updated to new value");

    println!("Update overwrites quote test passed!");
}

#[tokio::test]
async fn test_chain_toggle() {
    let mut pt = create_program_test();

    let payer = Keypair::new();
    let updater = get_updater_keypair();
    let chain_id: u16 = 2;
    let (chain_info_pda, chain_info_bump) = derive_chain_info_pda(chain_id);
    let (quote_body_pda, quote_body_bump) = derive_quote_body_pda(chain_id);
    let _payee_address = get_payee_address();
    let config_pubkey = get_dummy_config_pubkey();

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



    // Start with chain enabled
    let chain_info_data = create_chain_info_account_data(
        chain_info_bump,
        chain_id,
        true, // enabled
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

    // Disable the chain
    let disable_data = build_update_chain_info_data(chain_id, false, 9, 18);
    let disable_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &disable_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
            AccountMeta::new(chain_info_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );

    let mut tx = Transaction::new_with_payer(&[disable_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer, &updater], recent_blockhash);
    banks_client.process_transaction(tx).await.expect("Disable chain failed");

    // Verify chain is disabled
    let chain_info_account = banks_client
        .get_account(chain_info_pda)
        .await
        .expect("Failed to get account")
        .expect("ChainInfo account not found");
    // ChainInfo layout: discriminator (0), bump (1), chain_id (2-3), enabled (4)
    assert_eq!(chain_info_account.data[4], 0, "Chain should be disabled");

    // Try to request quote - should fail
    let recent_blockhash = banks_client.get_latest_blockhash().await.expect("get blockhash");
    let relay_instructions = build_relay_instructions_gas(200000, 0);
    let quote_data = build_request_quote_data(
        chain_id,
        &[0x01u8; 32],
        &[0x02u8; 32],
        &[],
        &relay_instructions,
    );
    let quote_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &quote_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );
    let mut tx = Transaction::new_with_payer(&[quote_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], recent_blockhash);
    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Quote should fail when chain disabled");

    // Re-enable the chain
    let recent_blockhash = banks_client.get_latest_blockhash().await.expect("get blockhash");
    let enable_data = build_update_chain_info_data(chain_id, true, 9, 18);
    let enable_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &enable_data,
        vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true),
            AccountMeta::new(chain_info_pda, false),
            AccountMeta::new_readonly(system_program::id(), false),
        ],
    );
    let mut tx = Transaction::new_with_payer(&[enable_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer, &updater], recent_blockhash);
    banks_client.process_transaction(tx).await.expect("Re-enable chain failed");

    // Verify chain is enabled again
    let chain_info_account = banks_client
        .get_account(chain_info_pda)
        .await
        .expect("Failed to get account")
        .expect("ChainInfo account not found");
    // ChainInfo layout: discriminator (0), bump (1), chain_id (2-3), enabled (4)
    assert_eq!(chain_info_account.data[4], 1, "Chain should be re-enabled");

    // Quote should work now
    let recent_blockhash = banks_client.get_latest_blockhash().await.expect("get blockhash");
    // Force account cache refresh - see solana-program-test behavior with BPF programs
    let _acc = banks_client.get_account(chain_info_pda).await.expect("get").expect("exists");
    let quote_ix = Instruction::new_with_bytes(
        PROGRAM_ID,
        &quote_data,
        vec![
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(chain_info_pda, false),
            AccountMeta::new_readonly(quote_body_pda, false),
        ],
    );
    let mut tx = Transaction::new_with_payer(&[quote_ix], Some(&payer.pubkey()));
    tx.sign(&[&payer], recent_blockhash);
    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_ok(), "Quote should succeed when chain re-enabled: {:?}", result.err());

    println!("Chain toggle test passed!");
}

// Note: test_account_data_layout removed - Config account no longer exists
// Values are now hardcoded in the program

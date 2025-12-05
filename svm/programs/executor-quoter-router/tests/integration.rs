//! Integration tests for executor-quoter-router using solana-program-test with BPF.
//!
//! These tests require both the executor-quoter-router and executor-quoter BPF binaries.

use libsecp256k1::{Message, PublicKey, SecretKey};
use rand::rngs::OsRng;
use solana_program_test::{tokio, ProgramTest, ProgramTestBanksClientExt};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::{AccountMeta, Instruction},
    keccak,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    system_program,
    transaction::Transaction,
};

/// Router Program ID - FgDLrWZ9avy9A4hNDLCvVUyh7knK9r2Ry4KgHX1y2aKS
const ROUTER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0xda, 0x0f, 0x39, 0x58, 0xba, 0x11, 0x3d, 0xfa, 0x31, 0xe1, 0xda, 0xc7, 0x67, 0xe7, 0x47, 0xce,
    0xc9, 0x03, 0xf4, 0x56, 0x9c, 0x89, 0x97, 0x1f, 0x47, 0x27, 0x2e, 0xb0, 0x7e, 0x3d, 0xd5, 0xf9,
]);

/// Quoter Program ID (matching executor-quoter)
const QUOTER_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x58, 0xce, 0x85, 0x6b, 0x53, 0xca, 0x8b, 0x7d, 0xc9, 0xa3, 0x84, 0x42, 0x1c, 0x5c, 0xaf, 0x30,
    0x63, 0xcf, 0x30, 0x96, 0x2b, 0x4c, 0xf6, 0x0d, 0xad, 0x51, 0x9d, 0x3d, 0xcd, 0xf3, 0x86, 0x58,
]);

/// Executor Program ID (placeholder)
const EXECUTOR_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x43, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02,
]);

// Account discriminators
const CONFIG_DISCRIMINATOR: u8 = 1;
const QUOTER_REGISTRATION_DISCRIMINATOR: u8 = 2;

// PDA seeds
const CONFIG_SEED: &[u8] = b"config";
const QUOTER_REGISTRATION_SEED: &[u8] = b"quoter_registration";

// Account sizes
const CONFIG_SIZE: usize = 40; // 1 + 1 + 2 + 4 + 32
const QUOTER_REGISTRATION_SIZE: usize = 56; // 1 + 1 + 2 + 20 + 32

// Instruction discriminators
const IX_INITIALIZE: u8 = 0;
const IX_UPDATE_QUOTER_CONTRACT: u8 = 1;
const IX_QUOTE_EXECUTION: u8 = 2;
const IX_REQUEST_EXECUTION: u8 = 3;

// Wormhole chain ID for Solana
const SOLANA_CHAIN_ID: u16 = 1;

/// Helper to derive config PDA
fn derive_config_pda() -> (Pubkey, u8) {
    Pubkey::find_program_address(&[CONFIG_SEED], &ROUTER_PROGRAM_ID)
}

/// Helper to derive quoter registration PDA
fn derive_quoter_registration_pda(quoter_address: &[u8; 20]) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[QUOTER_REGISTRATION_SEED, &quoter_address[..]],
        &ROUTER_PROGRAM_ID,
    )
}

/// Build Initialize instruction data
fn build_initialize_data(executor_program_id: &Pubkey, our_chain: u16, bump: u8) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 32 + 2 + 1);
    data.push(IX_INITIALIZE);
    data.extend_from_slice(executor_program_id.as_ref());
    data.extend_from_slice(&our_chain.to_le_bytes());
    data.push(bump);
    data
}

/// Secp256k1 quoter identity for testing.
/// Contains the secret key and derived Ethereum address.
struct QuoterIdentity {
    secret_key: SecretKey,
    eth_address: [u8; 20],
}

impl QuoterIdentity {
    /// Create a new random quoter identity.
    fn new() -> Self {
        let secret_key = SecretKey::random(&mut OsRng);
        let public_key = PublicKey::from_secret_key(&secret_key);

        // Derive Ethereum address: keccak256(pubkey)[12:32]
        // libsecp256k1 public key is 65 bytes with 0x04 prefix, we need the 64 bytes after
        let pubkey_bytes = public_key.serialize();
        let pubkey_hash = keccak::hash(&pubkey_bytes[1..65]);
        let mut eth_address = [0u8; 20];
        eth_address.copy_from_slice(&pubkey_hash.0[12..32]);

        Self {
            secret_key,
            eth_address,
        }
    }

    /// Sign a message and return (r, s, v).
    fn sign(&self, message: &[u8]) -> ([u8; 32], [u8; 32], u8) {
        let message_hash = keccak::hash(message);
        let message = Message::parse_slice(&message_hash.0).expect("valid message hash");

        let (signature, recovery_id) = libsecp256k1::sign(&message, &self.secret_key);
        let sig_bytes = signature.serialize();

        let mut r = [0u8; 32];
        let mut s = [0u8; 32];
        r.copy_from_slice(&sig_bytes[0..32]);
        s.copy_from_slice(&sig_bytes[32..64]);

        // EVM uses v = 27 or 28, recovery_id is 0 or 1
        let v = recovery_id.serialize() + 27;

        (r, s, v)
    }
}

/// Build a valid EG01 governance message with proper signature.
fn build_signed_governance_message(
    chain_id: u16,
    quoter: &QuoterIdentity,
    implementation_program_id: &Pubkey,
    sender: &Pubkey,
    expiry_time: u64,
) -> Vec<u8> {
    // Build the message body (bytes 0-98) that will be signed
    let mut body = Vec::with_capacity(98);
    body.extend_from_slice(b"EG01");
    body.extend_from_slice(&chain_id.to_be_bytes());
    body.extend_from_slice(&quoter.eth_address);
    body.extend_from_slice(implementation_program_id.as_ref()); // universal_contract_address
    body.extend_from_slice(sender.as_ref()); // universal_sender_address
    body.extend_from_slice(&expiry_time.to_be_bytes());

    assert_eq!(body.len(), 98, "Governance body should be 98 bytes");

    // Sign the body
    let (r, s, v) = quoter.sign(&body);

    // Build the full message
    let mut data = Vec::with_capacity(163);
    data.extend_from_slice(&body);
    data.extend_from_slice(&r);
    data.extend_from_slice(&s);
    data.push(v);

    assert_eq!(data.len(), 163, "Governance message should be 163 bytes");
    data
}

/// Build a valid EG01 governance message for testing (unsigned, for negative tests)
fn build_governance_message(
    chain_id: u16,
    quoter_address: &[u8; 20],
    implementation_program_id: &Pubkey,
    sender: &Pubkey,
    expiry_time: u64,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(163);
    data.extend_from_slice(b"EG01");
    data.extend_from_slice(&chain_id.to_be_bytes());
    data.extend_from_slice(quoter_address);
    data.extend_from_slice(implementation_program_id.as_ref()); // universal_contract_address
    data.extend_from_slice(sender.as_ref()); // universal_sender_address
    data.extend_from_slice(&expiry_time.to_be_bytes());
    // signature_r (32 bytes)
    data.extend_from_slice(&[0u8; 32]);
    // signature_s (32 bytes)
    data.extend_from_slice(&[0u8; 32]);
    // signature_v (1 byte)
    data.push(0);
    data
}

/// Build UpdateQuoterContract instruction data with proper signature
fn build_signed_update_quoter_contract_data(
    chain_id: u16,
    quoter: &QuoterIdentity,
    implementation_program_id: &Pubkey,
    sender: &Pubkey,
    expiry_time: u64,
    bump: u8,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 1 + 163);
    data.push(IX_UPDATE_QUOTER_CONTRACT);
    data.push(bump);
    data.extend(build_signed_governance_message(
        chain_id,
        quoter,
        implementation_program_id,
        sender,
        expiry_time,
    ));
    data
}

/// Build UpdateQuoterContract instruction data (unsigned, for negative tests)
fn build_update_quoter_contract_data(
    chain_id: u16,
    quoter_address: &[u8; 20],
    implementation_program_id: &Pubkey,
    sender: &Pubkey,
    expiry_time: u64,
    bump: u8,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 1 + 163);
    data.push(IX_UPDATE_QUOTER_CONTRACT);
    data.push(bump);
    data.extend(build_governance_message(
        chain_id,
        quoter_address,
        implementation_program_id,
        sender,
        expiry_time,
    ));
    data
}

/// Setup ProgramTest with router program
fn setup_program_test() -> ProgramTest {
    let mut pt = ProgramTest::default();

    // Add router program
    pt.add_program(
        "executor_quoter_router",
        ROUTER_PROGRAM_ID,
        None, // BPF loaded from target/deploy
    );

    // Force BPF execution for pinocchio programs
    pt.prefer_bpf(true);

    pt
}

/// Setup ProgramTest with both router and quoter programs
#[allow(dead_code)]
fn setup_program_test_with_quoter() -> ProgramTest {
    let mut pt = ProgramTest::default();

    // Add router program
    pt.add_program(
        "executor_quoter_router",
        ROUTER_PROGRAM_ID,
        None, // BPF loaded from target/deploy
    );

    // Add quoter program for CPI testing
    pt.add_program("executor_quoter", QUOTER_PROGRAM_ID, None);

    // Force BPF execution for pinocchio programs
    pt.prefer_bpf(true);

    pt
}

#[tokio::test]
async fn test_initialize() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, bump) = derive_config_pda();

    // Build initialize instruction
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, bump);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);

    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_ok(), "Initialize failed: {:?}", result);

    // Verify config account was created
    let config_account = banks_client.get_account(config_pda).await.unwrap();
    assert!(config_account.is_some(), "Config account not created");

    let config_data = config_account.unwrap().data;
    assert_eq!(config_data.len(), CONFIG_SIZE);
    assert_eq!(config_data[0], CONFIG_DISCRIMINATOR);
    assert_eq!(config_data[1], bump);

    // Verify our_chain (little-endian u16 at offset 2)
    let our_chain = u16::from_le_bytes([config_data[2], config_data[3]]);
    assert_eq!(our_chain, SOLANA_CHAIN_ID);

    // Verify executor_program_id (at offset 8 after padding)
    let stored_executor: [u8; 32] = config_data[8..40].try_into().unwrap();
    assert_eq!(stored_executor, EXECUTOR_PROGRAM_ID.to_bytes());
}

#[tokio::test]
async fn test_initialize_twice_fails() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, bump) = derive_config_pda();

    // First initialization
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);

    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    banks_client.process_transaction(tx).await.unwrap();

    // Second initialization should fail
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);

    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Second initialization should fail");
}

#[tokio::test]
async fn test_update_quoter_contract() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // First initialize the router
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Create a secp256k1 quoter identity
    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter.eth_address);

    // Create sender keypair (must match universal_sender_address)
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build signed UpdateQuoterContract instruction
    let expiry_time = u64::MAX;

    let ix_data = build_signed_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter,
        &QUOTER_PROGRAM_ID,
        &sender.pubkey(),
        expiry_time,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),         // payer
            AccountMeta::new_readonly(sender.pubkey(), true), // sender
            AccountMeta::new_readonly(config_pda, false),   // config
            AccountMeta::new(quoter_registration_pda, false), // quoter_registration
            AccountMeta::new_readonly(system_program::ID, false), // system_program
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_ok(),
        "UpdateQuoterContract failed: {:?}",
        result
    );

    // Verify quoter registration was created
    let registration_account = banks_client
        .get_account(quoter_registration_pda)
        .await
        .unwrap();
    assert!(
        registration_account.is_some(),
        "QuoterRegistration account not created"
    );

    let reg_data = registration_account.unwrap().data;
    assert_eq!(reg_data.len(), QUOTER_REGISTRATION_SIZE);
    assert_eq!(reg_data[0], QUOTER_REGISTRATION_DISCRIMINATOR);

    // Verify quoter_address (at offset 4 after discriminator, bump, padding)
    let stored_quoter_addr: [u8; 20] = reg_data[4..24].try_into().unwrap();
    assert_eq!(stored_quoter_addr, quoter.eth_address);

    // Verify implementation_program_id (at offset 24)
    let stored_impl: [u8; 32] = reg_data[24..56].try_into().unwrap();
    assert_eq!(stored_impl, QUOTER_PROGRAM_ID.to_bytes());
}

#[tokio::test]
async fn test_update_quoter_contract_wrong_chain_fails() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize the router with SOLANA_CHAIN_ID
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Try to register quoter with wrong chain ID (Ethereum = 2)
    let quoter_address: [u8; 20] = [0xAB; 20];
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter_address);

    let sender = Keypair::new();
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Use Ethereum chain ID (2) instead of Solana (1)
    let wrong_chain_id: u16 = 2;
    let ix_data = build_update_quoter_contract_data(
        wrong_chain_id,
        &quoter_address,
        &QUOTER_PROGRAM_ID,
        &sender.pubkey(),
        u64::MAX,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "Should fail with wrong chain ID"
    );
}

#[tokio::test]
async fn test_update_quoter_contract_expired_fails() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize the router
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Try to register quoter with expired timestamp
    let quoter_address: [u8; 20] = [0xAB; 20];
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter_address);

    let sender = Keypair::new();
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Use an already-expired timestamp
    let expired_time: u64 = 1; // Jan 1, 1970 + 1 second
    let ix_data = build_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter_address,
        &QUOTER_PROGRAM_ID,
        &sender.pubkey(),
        expired_time,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "Should fail with expired governance message"
    );
}

#[tokio::test]
async fn test_update_quoter_contract_wrong_sender_fails() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize the router
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Try to register with mismatched sender
    let quoter_address: [u8; 20] = [0xAB; 20];
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter_address);

    let sender = Keypair::new();
    let different_sender = Keypair::new();
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build message with different_sender's pubkey but sign with sender
    let ix_data = build_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter_address,
        &QUOTER_PROGRAM_ID,
        &different_sender.pubkey(), // Mismatch!
        u64::MAX,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true), // Actual signer
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "Should fail with sender mismatch"
    );
}

#[tokio::test]
async fn test_update_quoter_contract_update_existing() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize the router
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Create a secp256k1 quoter identity
    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter.eth_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let ix_data = build_signed_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter,
        &QUOTER_PROGRAM_ID,
        &sender.pubkey(),
        u64::MAX,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Verify first registration
    let reg_account = banks_client
        .get_account(quoter_registration_pda)
        .await
        .unwrap()
        .unwrap();
    let stored_impl: [u8; 32] = reg_account.data[24..56].try_into().unwrap();
    assert_eq!(stored_impl, QUOTER_PROGRAM_ID.to_bytes());

    // Now update to a different implementation (same quoter signs for different implementation)
    let new_implementation = Pubkey::new_from_array([0x99; 32]);

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let ix_data = build_signed_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter,
        &new_implementation,
        &sender.pubkey(),
        u64::MAX,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_ok(), "Update existing registration failed: {:?}", result);

    // Verify updated registration
    let reg_account = banks_client
        .get_account(quoter_registration_pda)
        .await
        .unwrap()
        .unwrap();
    let stored_impl: [u8; 32] = reg_account.data[24..56].try_into().unwrap();
    assert_eq!(stored_impl, new_implementation.to_bytes());
}

#[tokio::test]
async fn test_update_quoter_contract_bad_signature() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize the router
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Try with unsigned governance message (all zeros signature)
    let quoter_address: [u8; 20] = [0xAB; 20];
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // This uses the unsigned version (zeros for r, s, v)
    let ix_data = build_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter_address,
        &QUOTER_PROGRAM_ID,
        &sender.pubkey(),
        u64::MAX,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Should fail with bad signature");
}

#[tokio::test]
async fn test_update_quoter_contract_quoter_mismatch() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize the router
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Create two different quoter identities
    let quoter_alice = QuoterIdentity::new();
    let quoter_bob = QuoterIdentity::new();

    // Try to register Alice's address but sign with Bob's key
    // We need to manually construct this since our helper uses the same quoter for address and signing
    let sender = Keypair::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter_alice.eth_address);

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build message body with Alice's address
    let mut body = Vec::with_capacity(98);
    body.extend_from_slice(b"EG01");
    body.extend_from_slice(&SOLANA_CHAIN_ID.to_be_bytes());
    body.extend_from_slice(&quoter_alice.eth_address); // Alice's address
    body.extend_from_slice(QUOTER_PROGRAM_ID.as_ref());
    body.extend_from_slice(sender.pubkey().as_ref());
    body.extend_from_slice(&u64::MAX.to_be_bytes());

    // Sign with Bob's key
    let (r, s, v) = quoter_bob.sign(&body);

    let mut gov_data = Vec::with_capacity(163);
    gov_data.extend_from_slice(&body);
    gov_data.extend_from_slice(&r);
    gov_data.extend_from_slice(&s);
    gov_data.push(v);

    let mut ix_data = Vec::with_capacity(1 + 1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.push(quoter_bump);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Should fail with quoter mismatch (Alice's address, Bob's signature)");
}

#[tokio::test]
async fn test_update_quoter_contract_invalid_governance_prefix() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize the router
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Try to register with invalid governance prefix
    let quoter_address: [u8; 20] = [0xAB; 20];
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build governance message with wrong prefix
    let mut ix_data = Vec::with_capacity(1 + 1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.push(quoter_bump);
    ix_data.extend_from_slice(b"BAD!"); // Wrong prefix (should be "EG01")
    ix_data.extend_from_slice(&SOLANA_CHAIN_ID.to_be_bytes());
    ix_data.extend_from_slice(&quoter_address);
    ix_data.extend_from_slice(QUOTER_PROGRAM_ID.as_ref());
    ix_data.extend_from_slice(sender.pubkey().as_ref());
    ix_data.extend_from_slice(&u64::MAX.to_be_bytes());
    ix_data.extend_from_slice(&[0u8; 65]); // signature

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "Should fail with invalid governance prefix"
    );
}

// ============================================================================
// Ecrecover Unit Tests
// ============================================================================
//
// These tests verify the secp256k1 signature verification works correctly
// by testing various scenarios with known keys and signatures.

/// Test that a valid signature from a known key is accepted.
#[tokio::test]
async fn test_ecrecover_valid_signature() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize the router
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Create multiple quoter identities and verify each one works
    for i in 0..3 {
        let quoter = QuoterIdentity::new();
        let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter.eth_address);
        let sender = Keypair::new();

        let recent_blockhash = banks_client
            .get_new_latest_blockhash(&recent_blockhash)
            .await
            .unwrap();

        let ix_data = build_signed_update_quoter_contract_data(
            SOLANA_CHAIN_ID,
            &quoter,
            &QUOTER_PROGRAM_ID,
            &sender.pubkey(),
            u64::MAX,
            quoter_bump,
        );

        let ix = Instruction {
            program_id: ROUTER_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(sender.pubkey(), true),
                AccountMeta::new_readonly(config_pda, false),
                AccountMeta::new(quoter_registration_pda, false),
                AccountMeta::new_readonly(system_program::ID, false),
            ],
            data: ix_data,
        };

        let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
        let tx = Transaction::new_signed_with_payer(
            &[compute_ix, ix],
            Some(&payer.pubkey()),
            &[&payer, &sender],
            recent_blockhash,
        );

        let result = banks_client.process_transaction(tx).await;
        assert!(
            result.is_ok(),
            "Quoter {} with valid signature should succeed: {:?}",
            i,
            result
        );

        // Verify the registration exists with correct data
        let reg_account = banks_client
            .get_account(quoter_registration_pda)
            .await
            .unwrap()
            .unwrap();
        let stored_quoter_addr: [u8; 20] = reg_account.data[4..24].try_into().unwrap();
        assert_eq!(
            stored_quoter_addr, quoter.eth_address,
            "Stored quoter address should match"
        );
    }
}

/// Test that the same quoter can sign different messages (different implementations).
#[tokio::test]
async fn test_ecrecover_same_key_different_messages() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Same quoter, different implementations
    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter.eth_address);
    let sender = Keypair::new();

    // First registration
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let impl1 = Pubkey::new_from_array([0x11; 32]);
    let ix_data = build_signed_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter,
        &impl1,
        &sender.pubkey(),
        u64::MAX,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Verify first implementation
    let reg_account = banks_client
        .get_account(quoter_registration_pda)
        .await
        .unwrap()
        .unwrap();
    let stored_impl: [u8; 32] = reg_account.data[24..56].try_into().unwrap();
    assert_eq!(stored_impl, impl1.to_bytes());

    // Update to second implementation with new signature
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let impl2 = Pubkey::new_from_array([0x22; 32]);
    let ix_data = build_signed_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter,
        &impl2,
        &sender.pubkey(),
        u64::MAX,
        quoter_bump,
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Verify second implementation
    let reg_account = banks_client
        .get_account(quoter_registration_pda)
        .await
        .unwrap()
        .unwrap();
    let stored_impl: [u8; 32] = reg_account.data[24..56].try_into().unwrap();
    assert_eq!(stored_impl, impl2.to_bytes());
}

/// Test that recovery_id (v) must be correct - wrong v value should fail.
#[tokio::test]
async fn test_ecrecover_wrong_recovery_id() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter.eth_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build governance message body
    let mut body = Vec::with_capacity(98);
    body.extend_from_slice(b"EG01");
    body.extend_from_slice(&SOLANA_CHAIN_ID.to_be_bytes());
    body.extend_from_slice(&quoter.eth_address);
    body.extend_from_slice(QUOTER_PROGRAM_ID.as_ref());
    body.extend_from_slice(sender.pubkey().as_ref());
    body.extend_from_slice(&u64::MAX.to_be_bytes());

    // Sign correctly
    let (r, s, v) = quoter.sign(&body);

    // Flip the v value (if 27, make it 28; if 28, make it 27)
    let wrong_v = if v == 27 { 28 } else { 27 };

    // Build message with wrong v
    let mut gov_data = Vec::with_capacity(163);
    gov_data.extend_from_slice(&body);
    gov_data.extend_from_slice(&r);
    gov_data.extend_from_slice(&s);
    gov_data.push(wrong_v);

    let mut ix_data = Vec::with_capacity(1 + 1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.push(quoter_bump);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "Wrong recovery_id (v) should fail signature verification"
    );
}

/// Test that corrupted r value fails signature verification.
#[tokio::test]
async fn test_ecrecover_corrupted_r() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter.eth_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build and sign
    let mut body = Vec::with_capacity(98);
    body.extend_from_slice(b"EG01");
    body.extend_from_slice(&SOLANA_CHAIN_ID.to_be_bytes());
    body.extend_from_slice(&quoter.eth_address);
    body.extend_from_slice(QUOTER_PROGRAM_ID.as_ref());
    body.extend_from_slice(sender.pubkey().as_ref());
    body.extend_from_slice(&u64::MAX.to_be_bytes());

    let (mut r, s, v) = quoter.sign(&body);

    // Corrupt r by flipping a bit
    r[0] ^= 0x01;

    let mut gov_data = Vec::with_capacity(163);
    gov_data.extend_from_slice(&body);
    gov_data.extend_from_slice(&r);
    gov_data.extend_from_slice(&s);
    gov_data.push(v);

    let mut ix_data = Vec::with_capacity(1 + 1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.push(quoter_bump);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Corrupted r should fail");
}

/// Test that corrupted s value fails signature verification.
#[tokio::test]
async fn test_ecrecover_corrupted_s() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter.eth_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build and sign
    let mut body = Vec::with_capacity(98);
    body.extend_from_slice(b"EG01");
    body.extend_from_slice(&SOLANA_CHAIN_ID.to_be_bytes());
    body.extend_from_slice(&quoter.eth_address);
    body.extend_from_slice(QUOTER_PROGRAM_ID.as_ref());
    body.extend_from_slice(sender.pubkey().as_ref());
    body.extend_from_slice(&u64::MAX.to_be_bytes());

    let (r, mut s, v) = quoter.sign(&body);

    // Corrupt s by flipping a bit
    s[15] ^= 0xFF;

    let mut gov_data = Vec::with_capacity(163);
    gov_data.extend_from_slice(&body);
    gov_data.extend_from_slice(&r);
    gov_data.extend_from_slice(&s);
    gov_data.push(v);

    let mut ix_data = Vec::with_capacity(1 + 1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.push(quoter_bump);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Corrupted s should fail");
}

/// Test that modified message body fails (signature no longer valid).
#[tokio::test]
async fn test_ecrecover_modified_message() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter.eth_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build original message
    let mut body = Vec::with_capacity(98);
    body.extend_from_slice(b"EG01");
    body.extend_from_slice(&SOLANA_CHAIN_ID.to_be_bytes());
    body.extend_from_slice(&quoter.eth_address);
    body.extend_from_slice(QUOTER_PROGRAM_ID.as_ref());
    body.extend_from_slice(sender.pubkey().as_ref());
    body.extend_from_slice(&u64::MAX.to_be_bytes());

    // Sign the original message
    let (r, s, v) = quoter.sign(&body);

    // Modify the message body AFTER signing (change implementation)
    let mut modified_body = body.clone();
    modified_body[26] ^= 0x01; // Flip a bit in the implementation address

    // Build governance data with modified body but original signature
    let mut gov_data = Vec::with_capacity(163);
    gov_data.extend_from_slice(&modified_body);
    gov_data.extend_from_slice(&r);
    gov_data.extend_from_slice(&s);
    gov_data.push(v);

    let mut ix_data = Vec::with_capacity(1 + 1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.push(quoter_bump);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "Modified message with original signature should fail"
    );
}

/// Test with a known test vector to verify EVM compatibility.
/// This uses a deterministic key to ensure reproducible results.
#[tokio::test]
async fn test_ecrecover_deterministic_key() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let (config_pda, config_bump) = derive_config_pda();

    // Initialize
    let ix_data = build_initialize_data(&EXECUTOR_PROGRAM_ID, SOLANA_CHAIN_ID, config_bump);
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new(config_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Create quoter from deterministic secret key
    let secret_bytes: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e,
        0x1f, 0x20,
    ];
    let secret_key = SecretKey::parse(&secret_bytes).expect("valid secret key");
    let public_key = PublicKey::from_secret_key(&secret_key);

    // Derive Ethereum address
    let pubkey_bytes = public_key.serialize();
    let pubkey_hash = keccak::hash(&pubkey_bytes[1..65]);
    let mut eth_address = [0u8; 20];
    eth_address.copy_from_slice(&pubkey_hash.0[12..32]);

    // Log the derived address for verification
    // This can be compared with EVM ecrecover using the same key
    let hex_chars: Vec<String> = eth_address.iter().map(|b| format!("{:02x}", b)).collect();
    println!(
        "Deterministic key Ethereum address: 0x{}",
        hex_chars.join("")
    );

    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&eth_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build and sign governance message
    let mut body = Vec::with_capacity(98);
    body.extend_from_slice(b"EG01");
    body.extend_from_slice(&SOLANA_CHAIN_ID.to_be_bytes());
    body.extend_from_slice(&eth_address);
    body.extend_from_slice(QUOTER_PROGRAM_ID.as_ref());
    body.extend_from_slice(sender.pubkey().as_ref());
    body.extend_from_slice(&u64::MAX.to_be_bytes());

    // Sign
    let message_hash = keccak::hash(&body);
    let message = Message::parse_slice(&message_hash.0).expect("valid message hash");
    let (signature, recovery_id) = libsecp256k1::sign(&message, &secret_key);
    let sig_bytes = signature.serialize();

    let mut r = [0u8; 32];
    let mut s = [0u8; 32];
    r.copy_from_slice(&sig_bytes[0..32]);
    s.copy_from_slice(&sig_bytes[32..64]);
    let v = recovery_id.serialize() + 27;

    let mut gov_data = Vec::with_capacity(163);
    gov_data.extend_from_slice(&body);
    gov_data.extend_from_slice(&r);
    gov_data.extend_from_slice(&s);
    gov_data.push(v);

    let mut ix_data = Vec::with_capacity(1 + 1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.push(quoter_bump);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_ok(),
        "Deterministic key signature should succeed: {:?}",
        result
    );

    // Verify registration
    let reg_account = banks_client
        .get_account(quoter_registration_pda)
        .await
        .unwrap()
        .unwrap();
    let stored_quoter_addr: [u8; 20] = reg_account.data[4..24].try_into().unwrap();
    assert_eq!(stored_quoter_addr, eth_address);
}

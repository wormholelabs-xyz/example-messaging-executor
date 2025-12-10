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

/// Executor Program ID - execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV
const EXECUTOR_PROGRAM_ID: Pubkey = Pubkey::new_from_array([
    0x09, 0xb9, 0x69, 0x71, 0x58, 0x3b, 0x59, 0x03, 0xe0, 0x28, 0x1d, 0xa9, 0x65, 0x48, 0xd5, 0xd2,
    0x3c, 0x65, 0x1f, 0x7a, 0x9c, 0xcd, 0xe3, 0xea, 0xd5, 0x2b, 0x42, 0xf6, 0xb7, 0xda, 0xc2, 0xd2,
]);

// Account discriminators (updated - no config)
const QUOTER_REGISTRATION_DISCRIMINATOR: u8 = 1;

// PDA seeds
const QUOTER_REGISTRATION_SEED: &[u8] = b"quoter_registration";

// Account sizes
const QUOTER_REGISTRATION_SIZE: usize = 54; // 1 + 1 + 20 + 32

// Instruction discriminators (updated - no initialize)
const IX_UPDATE_QUOTER_CONTRACT: u8 = 0;
const IX_QUOTE_EXECUTION: u8 = 1;
const IX_REQUEST_EXECUTION: u8 = 2;

// Wormhole chain ID for Solana
const SOLANA_CHAIN_ID: u16 = 1;

/// Helper to get a dummy config pubkey (not used by program but required in instruction)
fn get_dummy_config_pubkey() -> Pubkey {
    Pubkey::new_unique()
}

/// Helper to derive quoter registration PDA
fn derive_quoter_registration_pda(quoter_address: &[u8; 20]) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[QUOTER_REGISTRATION_SEED, &quoter_address[..]],
        &ROUTER_PROGRAM_ID,
    )
}

// Note: build_initialize_data removed - program no longer has Initialize instruction

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
) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 163);
    data.push(IX_UPDATE_QUOTER_CONTRACT);
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
) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + 163);
    data.push(IX_UPDATE_QUOTER_CONTRACT);
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

/// Setup ProgramTest with router, quoter, and executor programs
fn setup_program_test_full() -> ProgramTest {
    let mut pt = ProgramTest::default();

    // Add router program
    pt.add_program(
        "executor_quoter_router",
        ROUTER_PROGRAM_ID,
        None,
    );

    // Add quoter program for CPI testing
    pt.add_program("executor_quoter", QUOTER_PROGRAM_ID, None);

    // Add executor program for full flow testing
    pt.add_program("executor", EXECUTOR_PROGRAM_ID, None);

    // Force BPF execution for pinocchio programs
    pt.prefer_bpf(true);

    pt
}

#[tokio::test]
async fn test_update_quoter_contract() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),         // payer
            AccountMeta::new_readonly(sender.pubkey(), true), // sender
            AccountMeta::new_readonly(config_pubkey, false),   // config
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
    let stored_quoter_addr: [u8; 20] = reg_data[2..22].try_into().unwrap();
    assert_eq!(stored_quoter_addr, quoter.eth_address);

    // Verify implementation_program_id (at offset 24)
    let stored_impl: [u8; 32] = reg_data[22..54].try_into().unwrap();
    assert_eq!(stored_impl, QUOTER_PROGRAM_ID.to_bytes());
}

#[tokio::test]
async fn test_update_quoter_contract_wrong_chain_fails() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true), // Actual signer
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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
    let stored_impl: [u8; 32] = reg_account.data[22..54].try_into().unwrap();
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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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
    let stored_impl: [u8; 32] = reg_account.data[22..54].try_into().unwrap();
    assert_eq!(stored_impl, new_implementation.to_bytes());
}

#[tokio::test]
async fn test_update_quoter_contract_bad_signature() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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

    let mut ix_data = Vec::with_capacity(1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

    // Try to register with invalid governance prefix
    let quoter_address: [u8; 20] = [0xAB; 20];
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build governance message with wrong prefix
    let mut ix_data = Vec::with_capacity(1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
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
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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
        );

        let ix = Instruction {
            program_id: ROUTER_PROGRAM_ID,
            accounts: vec![
                AccountMeta::new(payer.pubkey(), true),
                AccountMeta::new_readonly(sender.pubkey(), true),
                AccountMeta::new_readonly(config_pubkey, false),
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
        let stored_quoter_addr: [u8; 20] = reg_account.data[2..22].try_into().unwrap();
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

    let config_pubkey = get_dummy_config_pubkey();

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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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
    let stored_impl: [u8; 32] = reg_account.data[22..54].try_into().unwrap();
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
    );

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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
    let stored_impl: [u8; 32] = reg_account.data[22..54].try_into().unwrap();
    assert_eq!(stored_impl, impl2.to_bytes());
}

/// Test that recovery_id (v) must be correct - wrong v value should fail.
#[tokio::test]
async fn test_ecrecover_wrong_recovery_id() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

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

    let mut ix_data = Vec::with_capacity(1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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

    let mut ix_data = Vec::with_capacity(1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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

    let mut ix_data = Vec::with_capacity(1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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

    let mut ix_data = Vec::with_capacity(1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    let config_pubkey = get_dummy_config_pubkey();

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

    let mut ix_data = Vec::with_capacity(1 + 163);
    ix_data.push(IX_UPDATE_QUOTER_CONTRACT);
    ix_data.extend(gov_data);

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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
    let stored_quoter_addr: [u8; 20] = reg_account.data[2..22].try_into().unwrap();
    assert_eq!(stored_quoter_addr, eth_address);
}

// ============================================================================
// QuoteExecution and RequestExecution Tests
// ============================================================================

// Quoter PDA seeds
const QUOTER_CHAIN_INFO_SEED: &[u8] = b"chain_info";
const QUOTER_QUOTE_SEED: &[u8] = b"quote";

/// Derive quoter chain_info PDA
fn derive_quoter_chain_info_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[QUOTER_CHAIN_INFO_SEED, &chain_id.to_le_bytes()],
        &QUOTER_PROGRAM_ID,
    )
}

/// Derive quoter quote_body PDA
fn derive_quoter_quote_body_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[QUOTER_QUOTE_SEED, &chain_id.to_le_bytes()],
        &QUOTER_PROGRAM_ID,
    )
}

/// Quoter updater address (hardcoded in quoter program) - 9r6q2iEg4MBevjC8reaLmQUDxueF3vabUoqDkZ2LoAYe
const QUOTER_UPDATER_ADDRESS: Pubkey = Pubkey::new_from_array([
    0x83, 0x71, 0x8b, 0x7e, 0xc8, 0x96, 0x17, 0xb7,
    0x04, 0x06, 0x85, 0xe0, 0x1b, 0xdc, 0xca, 0x03,
    0x21, 0x40, 0x22, 0x98, 0x0d, 0xaa, 0xe9, 0x13,
    0x40, 0xe0, 0xc3, 0xf8, 0x40, 0xc0, 0x05, 0xef,
]);

/// Helper to get a dummy config pubkey for the quoter program (not used by program)
fn get_quoter_dummy_config_pubkey() -> Pubkey {
    Pubkey::new_unique()
}

/// Get the authorized updater keypair for quoter program.
/// Reads from QUOTER_UPDATER_KEYPAIR_PATH env var (path to JSON keypair file).
fn get_quoter_updater_keypair() -> Keypair {
    let keypair_path = std::env::var("QUOTER_UPDATER_KEYPAIR_PATH")
        .expect("QUOTER_UPDATER_KEYPAIR_PATH env var must be set to path of updater keypair JSON file");
    solana_sdk::signature::read_keypair_file(&keypair_path)
        .expect("Failed to read updater keypair from file")
}

// Quoter instruction discriminators
// Admin instructions (0, 1): 1-byte discriminator for minimal tx size
const QUOTER_IX_UPDATE_CHAIN_INFO: u8 = 0;
const QUOTER_IX_UPDATE_QUOTE: u8 = 1;
// Note: QUOTER_IX_REQUEST_QUOTE and QUOTER_IX_REQUEST_EXECUTION_QUOTE
// are defined above as IX_QUOTER_REQUEST_QUOTE and IX_QUOTER_REQUEST_EXECUTION_QUOTE

/// Build relay instructions with gas limit and msg value (Type 1)
fn build_relay_instructions_gas(gas_limit: u128, msg_value: u128) -> Vec<u8> {
    let mut data = Vec::with_capacity(33);
    data.push(1); // IX_TYPE_GAS
    data.extend_from_slice(&gas_limit.to_be_bytes());
    data.extend_from_slice(&msg_value.to_be_bytes());
    data
}

/// Quoter discriminator for RequestQuote (8 bytes, Anchor-compatible)
/// Byte 0 = instruction ID, bytes 1-7 = padding (zeros)
const IX_QUOTER_REQUEST_QUOTE: [u8; 8] = [2, 0, 0, 0, 0, 0, 0, 0];
/// Quoter discriminator for RequestExecutionQuote (8 bytes, Anchor-compatible)
/// Byte 0 = instruction ID, bytes 1-7 = padding (zeros)
const IX_QUOTER_REQUEST_EXECUTION_QUOTE: [u8; 8] = [3, 0, 0, 0, 0, 0, 0, 0];

/// Build QuoteExecution instruction data (zero-copy layout).
///
/// Layout:
/// - router_discriminator (1)
/// - quoter_address (20) - for registration lookup
/// - quoter CPI data:
///   - quoter_discriminator (8) - Anchor-compatible, byte 0 = RequestQuote (2)
///   - dst_chain (2)
///   - dst_addr (32)
///   - refund_addr (32)
///   - request_bytes_len (4)
///   - request_bytes
///   - relay_instructions_len (4)
///   - relay_instructions
fn build_quote_execution_data(
    quoter_address: &[u8; 20],
    dst_chain: u16,
    dst_addr: &[u8; 32],
    refund_addr: &[u8; 32],
    request_bytes: &[u8],
    relay_instructions: &[u8],
) -> Vec<u8> {
    let mut data = Vec::new();
    // Router discriminator
    data.push(IX_QUOTE_EXECUTION);
    // Quoter address for registration lookup
    data.extend_from_slice(quoter_address);
    // Quoter CPI data (passed directly to quoter)
    data.extend_from_slice(&IX_QUOTER_REQUEST_QUOTE); // 8-byte quoter discriminator
    data.extend_from_slice(&dst_chain.to_le_bytes());
    data.extend_from_slice(dst_addr);
    data.extend_from_slice(refund_addr);
    data.extend_from_slice(&(request_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(request_bytes);
    data.extend_from_slice(&(relay_instructions.len() as u32).to_le_bytes());
    data.extend_from_slice(relay_instructions);
    data
}

/// Build RequestExecution instruction data (zero-copy layout).
///
/// Layout:
/// - router_discriminator (1)
/// - amount (8) - payment amount
/// - quoter_address (20) - for registration lookup
/// - quoter CPI data:
///   - quoter_discriminator (8) - Anchor-compatible, byte 0 = RequestExecutionQuote (3)
///   - dst_chain (2)
///   - dst_addr (32)
///   - refund_addr (32)
///   - request_bytes_len (4)
///   - request_bytes
///   - relay_instructions_len (4)
///   - relay_instructions
fn build_request_execution_data(
    quoter_address: &[u8; 20],
    amount: u64,
    dst_chain: u16,
    dst_addr: &[u8; 32],
    refund_addr: &[u8; 32],
    request_bytes: &[u8],
    relay_instructions: &[u8],
) -> Vec<u8> {
    let mut data = Vec::new();
    // Router discriminator
    data.push(IX_REQUEST_EXECUTION);
    // Amount for payment
    data.extend_from_slice(&amount.to_le_bytes());
    // Quoter address for registration lookup
    data.extend_from_slice(quoter_address);
    // Quoter CPI data (passed directly to quoter)
    data.extend_from_slice(&IX_QUOTER_REQUEST_EXECUTION_QUOTE); // 8-byte quoter discriminator
    data.extend_from_slice(&dst_chain.to_le_bytes());
    data.extend_from_slice(dst_addr);
    data.extend_from_slice(refund_addr);
    data.extend_from_slice(&(request_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(request_bytes);
    data.extend_from_slice(&(relay_instructions.len() as u32).to_le_bytes());
    data.extend_from_slice(relay_instructions);
    data
}

/// Test destination chain ID
const DST_CHAIN_ID: u16 = 2; // Ethereum

#[tokio::test]
async fn test_quote_execution() {
    let pt = setup_program_test_with_quoter();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    // Register quoter
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
    );
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    // Setup quoter accounts (config, chain_info, quote_body)
    // Note: We need to initialize the quoter program's accounts
    // For this test, we'll use the quoter's initialize instruction

    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _quoter_chain_info_bump) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _quoter_quote_body_bump) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let updater = get_quoter_updater_keypair();

    // Update chain info
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, chain_info_bump) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let mut chain_info_data = Vec::new();
    chain_info_data.push(QUOTER_IX_UPDATE_CHAIN_INFO); // 1-byte discriminator
    chain_info_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    chain_info_data.push(1); // enabled
    chain_info_data.push(9); // gas_price_decimals
    chain_info_data.push(18); // native_decimals
    chain_info_data.push(chain_info_bump);
    chain_info_data.extend_from_slice(&[0u8; 2]); // padding

    let chain_info_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: chain_info_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, chain_info_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Update quote
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, quote_body_bump) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let mut quote_data = Vec::new();
    quote_data.push(QUOTER_IX_UPDATE_QUOTE); // 1-byte discriminator
    quote_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    quote_data.push(quote_body_bump);
    quote_data.extend_from_slice(&[0u8; 5]); // padding
    quote_data.extend_from_slice(&2000_0000000000u64.to_le_bytes()); // dst_price
    quote_data.extend_from_slice(&200_0000000000u64.to_le_bytes()); // src_price
    quote_data.extend_from_slice(&50_000000000u64.to_le_bytes()); // dst_gas_price (50 Gwei)
    quote_data.extend_from_slice(&1000000u64.to_le_bytes()); // base_fee

    let quote_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: quote_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, quote_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Now call QuoteExecution through the router
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let dst_addr = [0x01u8; 32];
    let refund_addr = [0x02u8; 32];
    let relay_instructions = build_relay_instructions_gas(200000, 0);

    let quote_ix_data = build_quote_execution_data(
        &quoter.eth_address,
        DST_CHAIN_ID,
        &dst_addr,
        &refund_addr,
        &[],
        &relay_instructions,
    );

    let quote_ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
        ],
        data: quote_ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(300_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, quote_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_ok(), "QuoteExecution failed: {:?}", result);
}

#[tokio::test]
async fn test_quote_execution_quoter_not_registered() {
    let pt = setup_program_test_with_quoter();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    // Register quoter A
    let quoter_a = QuoterIdentity::new();
    let (quoter_registration_pda, quoter_bump) = derive_quoter_registration_pda(&quoter_a.eth_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let ix_data = build_signed_update_quoter_contract_data(
        SOLANA_CHAIN_ID,
        &quoter_a,
        &QUOTER_PROGRAM_ID,
        &sender.pubkey(),
        u64::MAX,
    );
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    // Setup quoter accounts
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);

    // Try to call QuoteExecution with a different quoter address (quoter B)
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let quoter_b = QuoterIdentity::new();
    let dst_addr = [0x01u8; 32];
    let refund_addr = [0x02u8; 32];
    let relay_instructions = build_relay_instructions_gas(200000, 0);

    // Use quoter_b's address but quoter_a's registration PDA
    let quote_ix_data = build_quote_execution_data(
        &quoter_b.eth_address, // Different quoter address!
        DST_CHAIN_ID,
        &dst_addr,
        &refund_addr,
        &[],
        &relay_instructions,
    );

    let quote_ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(quoter_registration_pda, false), // quoter_a's registration
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
        ],
        data: quote_ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(300_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, quote_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "QuoteExecution should fail with mismatched quoter address"
    );
}

#[tokio::test]
async fn test_quote_execution_chain_disabled() {
    let pt = setup_program_test_with_quoter();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    // Register quoter
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
    );
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    // Setup quoter accounts but with chain DISABLED
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let updater = get_quoter_updater_keypair();

    // Update chain info with enabled = FALSE
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, chain_info_bump) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let mut chain_info_data = Vec::new();
    chain_info_data.push(QUOTER_IX_UPDATE_CHAIN_INFO); // 1-byte discriminator
    chain_info_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    chain_info_data.push(0); // enabled = FALSE (chain disabled)
    chain_info_data.push(9); // gas_price_decimals
    chain_info_data.push(18); // native_decimals
    chain_info_data.push(chain_info_bump);
    chain_info_data.extend_from_slice(&[0u8; 2]); // padding

    let chain_info_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: chain_info_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, chain_info_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Update quote (needed even for disabled chain test)
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, quote_body_bump) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let mut quote_data = Vec::new();
    quote_data.push(QUOTER_IX_UPDATE_QUOTE); // 1-byte discriminator
    quote_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    quote_data.push(quote_body_bump);
    quote_data.extend_from_slice(&[0u8; 5]); // padding
    quote_data.extend_from_slice(&2000_0000000000u64.to_le_bytes());
    quote_data.extend_from_slice(&200_0000000000u64.to_le_bytes());
    quote_data.extend_from_slice(&50_000000000u64.to_le_bytes());
    quote_data.extend_from_slice(&1000000u64.to_le_bytes());

    let quote_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: quote_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, quote_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Now call QuoteExecution - should fail because chain is disabled
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let dst_addr = [0x01u8; 32];
    let refund_addr = [0x02u8; 32];
    let relay_instructions = build_relay_instructions_gas(200000, 0);

    let quote_ix_data = build_quote_execution_data(
        &quoter.eth_address,
        DST_CHAIN_ID,
        &dst_addr,
        &refund_addr,
        &[],
        &relay_instructions,
    );

    let quote_ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
        ],
        data: quote_ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(300_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, quote_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "QuoteExecution should fail when chain is disabled"
    );
}

#[tokio::test]
async fn test_request_execution() {
    let pt = setup_program_test_full();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    // Register quoter
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
    );
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    // Setup quoter accounts
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let updater = get_quoter_updater_keypair();

    // Payee is the hardcoded PAYEE_ADDRESS in the quoter program
    // This must match what the quoter returns via CPI (updater.pubkey() when built
    // with QUOTER_PAYEE_PUBKEY=QUOTER_UPDATER_PUBKEY)
    let payee_pubkey = updater.pubkey();

    // Update chain info
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, chain_info_bump) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let mut chain_info_data = Vec::new();
    chain_info_data.push(QUOTER_IX_UPDATE_CHAIN_INFO); // 1-byte discriminator
    chain_info_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    chain_info_data.push(1); // enabled
    chain_info_data.push(12); // gas_price_decimals (matching EVM tests: 0x12 = 18, but the hex update shows 12)
    chain_info_data.push(18); // native_decimals (ETH = 18)
    chain_info_data.push(chain_info_bump);
    chain_info_data.extend_from_slice(&[0u8; 2]); // padding

    let chain_info_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: chain_info_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, chain_info_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Update quote
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, quote_body_bump) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let mut quote_data = Vec::new();
    quote_data.push(QUOTER_IX_UPDATE_QUOTE); // 1-byte discriminator
    quote_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    quote_data.push(quote_body_bump);
    quote_data.extend_from_slice(&[0u8; 5]); // padding
    quote_data.extend_from_slice(&35751300000000u64.to_le_bytes()); // dst_price (matching EVM tests)
    quote_data.extend_from_slice(&35751300000000u64.to_le_bytes()); // src_price (same as dst for 1:1 ratio)
    quote_data.extend_from_slice(&100000000u64.to_le_bytes()); // dst_gas_price (0.1 gwei, matching EVM)
    quote_data.extend_from_slice(&27971u64.to_le_bytes()); // base_fee (matching EVM tests)

    let quote_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: quote_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, quote_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Now call RequestExecution
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let dst_addr = [0x01u8; 32];
    let refund_addr = payer.pubkey().to_bytes();
    let relay_instructions = build_relay_instructions_gas(250000, 0); // 250k gas, matching EVM tests

    // Use a large enough amount to cover the quote - EVM test uses 27797100000000 wei
    // For our test with 1:1 price ratio, let's use a generous amount
    let amount: u64 = 100_000_000_000; // 100 SOL (way more than needed)

    let request_ix_data = build_request_execution_data(
        &quoter.eth_address,
        amount,
        DST_CHAIN_ID,
        &dst_addr,
        &refund_addr,
        &[],
        &relay_instructions,
    );

    // The event_cpi account is required but not used - use a placeholder
    let event_cpi = Pubkey::new_unique();

    let request_ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),      // payer
            AccountMeta::new_readonly(config_pubkey, false), // config
            AccountMeta::new_readonly(quoter_registration_pda, false), // quoter_registration
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false), // quoter_program
            AccountMeta::new_readonly(EXECUTOR_PROGRAM_ID, false), // executor_program
            AccountMeta::new(payee_pubkey, false),     // payee
            AccountMeta::new(payer.pubkey(), false),     // refund_addr
            AccountMeta::new_readonly(system_program::ID, false), // system_program
            AccountMeta::new_readonly(quoter_config_pubkey, false), // quoter_config
            AccountMeta::new_readonly(quoter_chain_info_pda, false), // quoter_chain_info
            AccountMeta::new_readonly(quoter_quote_body_pda, false), // quoter_quote_body
            AccountMeta::new_readonly(event_cpi, false), // event_cpi
        ],
        data: request_ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, request_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_ok(), "RequestExecution failed: {:?}", result);

    // Verify payee received payment
    let payee_account = banks_client.get_account(payee_pubkey).await.unwrap();
    assert!(
        payee_account.is_some(),
        "Payee account should exist after payment"
    );
    assert!(
        payee_account.unwrap().lamports > 0,
        "Payee should have received payment"
    );
}

#[tokio::test]
async fn test_request_execution_underpaid() {
    let pt = setup_program_test_full();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    // Register quoter
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
    );
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    // Setup quoter accounts
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let updater = get_quoter_updater_keypair();

    let payee = Keypair::new();
    let payee_address_bytes: [u8; 32] = payee.pubkey().to_bytes();

    // Update chain info
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, chain_info_bump) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let mut chain_info_data = Vec::new();
    chain_info_data.push(QUOTER_IX_UPDATE_CHAIN_INFO); // 1-byte discriminator
    chain_info_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    chain_info_data.push(1);
    chain_info_data.push(9);
    chain_info_data.push(18);
    chain_info_data.push(chain_info_bump);
    chain_info_data.extend_from_slice(&[0u8; 2]);

    let chain_info_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: chain_info_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, chain_info_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Update quote with a high base_fee to ensure the quote is high
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, quote_body_bump) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let mut quote_data = Vec::new();
    quote_data.push(QUOTER_IX_UPDATE_QUOTE); // 1-byte discriminator
    quote_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    quote_data.push(quote_body_bump);
    quote_data.extend_from_slice(&[0u8; 5]);
    quote_data.extend_from_slice(&2000_0000000000u64.to_le_bytes());
    quote_data.extend_from_slice(&200_0000000000u64.to_le_bytes());
    quote_data.extend_from_slice(&50_000000000u64.to_le_bytes());
    quote_data.extend_from_slice(&1_000_000_000u64.to_le_bytes()); // high base_fee = 1 SOL

    let quote_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: quote_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, quote_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Call RequestExecution with insufficient payment
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let dst_addr = [0x01u8; 32];
    let refund_addr = payer.pubkey().to_bytes();
    let relay_instructions = build_relay_instructions_gas(200000, 0);

    // Use a very small amount that will definitely be less than the quote
    let amount: u64 = 1000; // Only 1000 lamports

    let request_ix_data = build_request_execution_data(
        &quoter.eth_address,
        amount,
        DST_CHAIN_ID,
        &dst_addr,
        &refund_addr,
        &[],
        &relay_instructions,
    );

    let event_cpi = Pubkey::new_unique();

    let request_ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(EXECUTOR_PROGRAM_ID, false),
            AccountMeta::new(payee.pubkey(), false),
            AccountMeta::new(payer.pubkey(), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(event_cpi, false),
        ],
        data: request_ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, request_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    assert!(
        result.is_err(),
        "RequestExecution should fail when underpaid"
    );
}

// ============================================================================
// Boundary Condition Tests
// ============================================================================

// Note: Signer verification tests are omitted because the Solana runtime
// handles signature verification before the program is invoked. Testing
// missing signatures would only test the SDK/runtime, not our program logic.

// --- UpdateQuoterContract Boundary Tests ---

#[tokio::test]
async fn test_update_quoter_contract_empty_data() {
    let pt = setup_program_test();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    // Try UpdateQuoterContract with empty data
    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, _) = derive_quoter_registration_pda(&quoter.eth_address);
    let sender = Keypair::new();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: vec![1], // Only discriminator
    };

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&payer.pubkey()),
        &[&payer, &sender],
        recent_blockhash,
    );
    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Should fail with empty governance data");
}

// Note: test_update_quoter_contract_invalid_prefix is covered by
// test_update_quoter_contract_invalid_governance_prefix above.
// Note: test_update_quoter_contract_expiry_exactly_now is covered by
// test_update_quoter_contract_expired_fails above.

// --- QuoteExecution Boundary Tests ---

#[tokio::test]
async fn test_quote_execution_empty_data() {
    let pt = setup_program_test_full();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, _) = derive_quoter_registration_pda(&quoter.eth_address);
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // QuoteExecution with empty data
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
        ],
        data: vec![2], // Only discriminator
    };

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], recent_blockhash);
    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Should fail with empty QuoteExecution data");
}

#[tokio::test]
async fn test_quote_execution_partial_data() {
    let pt = setup_program_test_full();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, _) = derive_quoter_registration_pda(&quoter.eth_address);
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // QuoteExecution with only 50 bytes (needs 94 minimum)
    let mut ix_data = vec![2u8]; // QuoteExecution discriminator
    ix_data.extend_from_slice(&[0u8; 50]); // Not enough data

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], recent_blockhash);
    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Should fail with partial QuoteExecution data");
}

// --- RequestExecution Boundary Tests ---

#[tokio::test]
async fn test_request_execution_empty_data() {
    let pt = setup_program_test_full();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, _) = derive_quoter_registration_pda(&quoter.eth_address);
    let payee = Keypair::new();
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let event_cpi = Pubkey::new_unique();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // RequestExecution with empty data
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(EXECUTOR_PROGRAM_ID, false),
            AccountMeta::new(payee.pubkey(), false),
            AccountMeta::new(payer.pubkey(), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(event_cpi, false),
        ],
        data: vec![3], // Only discriminator
    };

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], recent_blockhash);
    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Should fail with empty RequestExecution data");
}

#[tokio::test]
async fn test_request_execution_amount_zero() {
    let pt = setup_program_test_full();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    // Register quoter
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
    );
    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(sender.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
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

    // Setup quoter accounts
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let payee = Keypair::new();
    let payee_address_bytes: [u8; 32] = payee.pubkey().to_bytes();
    let updater = get_quoter_updater_keypair();

    // Update chain info
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, chain_info_bump) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let mut chain_info_data = Vec::new();
    chain_info_data.push(QUOTER_IX_UPDATE_CHAIN_INFO); // 1-byte discriminator
    chain_info_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    chain_info_data.push(1);
    chain_info_data.push(12);
    chain_info_data.push(18);
    chain_info_data.push(chain_info_bump);
    chain_info_data.extend_from_slice(&[0u8; 2]);

    let chain_info_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: chain_info_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, chain_info_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // Update quote with non-zero base_fee so quote > 0
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let (_, quote_body_bump) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let mut quote_data = Vec::new();
    quote_data.push(QUOTER_IX_UPDATE_QUOTE); // 1-byte discriminator
    quote_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes());
    quote_data.push(quote_body_bump);
    quote_data.extend_from_slice(&[0u8; 5]);
    quote_data.extend_from_slice(&35751300000000u64.to_le_bytes());
    quote_data.extend_from_slice(&35751300000000u64.to_le_bytes());
    quote_data.extend_from_slice(&100000000u64.to_le_bytes());
    quote_data.extend_from_slice(&27971u64.to_le_bytes());

    let quote_ix = Instruction {
        program_id: QUOTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(updater.pubkey(), true), // updater
            AccountMeta::new(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
        data: quote_data,
    };
    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(200_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, quote_ix],
        Some(&payer.pubkey()),
        &[&payer, &updater],
        recent_blockhash,
    );
    banks_client.process_transaction(tx).await.unwrap();

    // RequestExecution with amount = 0
    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    let dst_addr = [0x01u8; 32];
    let refund_addr = payer.pubkey().to_bytes();
    let relay_instructions = build_relay_instructions_gas(250000, 0);

    let request_ix_data = build_request_execution_data(
        &quoter.eth_address,
        0, // Zero amount!
        DST_CHAIN_ID,
        &dst_addr,
        &refund_addr,
        &[],
        &relay_instructions,
    );

    let event_cpi = Pubkey::new_unique();

    let request_ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(EXECUTOR_PROGRAM_ID, false),
            AccountMeta::new(payee.pubkey(), false),
            AccountMeta::new(payer.pubkey(), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(event_cpi, false),
        ],
        data: request_ix_data,
    };

    let compute_ix = ComputeBudgetInstruction::set_compute_unit_limit(400_000);
    let tx = Transaction::new_signed_with_payer(
        &[compute_ix, request_ix],
        Some(&payer.pubkey()),
        &[&payer],
        recent_blockhash,
    );

    let result = banks_client.process_transaction(tx).await;
    // Should fail because quote requires payment but amount is 0
    assert!(result.is_err(), "Should fail with zero amount when quote requires payment");
}

#[tokio::test]
async fn test_request_execution_max_request_bytes_len() {
    let pt = setup_program_test_full();
    let (mut banks_client, payer, recent_blockhash) = pt.start().await;

    let config_pubkey = get_dummy_config_pubkey();

    let quoter = QuoterIdentity::new();
    let (quoter_registration_pda, _) = derive_quoter_registration_pda(&quoter.eth_address);
    let payee = Keypair::new();
    let quoter_config_pubkey = get_quoter_dummy_config_pubkey();
    let (quoter_chain_info_pda, _) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, _) = derive_quoter_quote_body_pda(DST_CHAIN_ID);
    let event_cpi = Pubkey::new_unique();

    let recent_blockhash = banks_client
        .get_new_latest_blockhash(&recent_blockhash)
        .await
        .unwrap();

    // Build request with request_bytes_len = u32::MAX but no actual data
    // This should trigger a bounds check failure
    let mut ix_data = vec![3u8]; // RequestExecution discriminator
    ix_data.extend_from_slice(&[0u8; 20]); // quoter_address
    ix_data.extend_from_slice(&100u64.to_le_bytes()); // amount
    ix_data.extend_from_slice(&DST_CHAIN_ID.to_le_bytes()); // dst_chain
    ix_data.extend_from_slice(&[0u8; 32]); // dst_addr
    ix_data.extend_from_slice(&[0u8; 32]); // refund_addr
    ix_data.extend_from_slice(&u32::MAX.to_le_bytes()); // request_bytes_len = MAX
    // No actual request_bytes - this should fail bounds check

    let ix = Instruction {
        program_id: ROUTER_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(payer.pubkey(), true),
            AccountMeta::new_readonly(config_pubkey, false),
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(EXECUTOR_PROGRAM_ID, false),
            AccountMeta::new(payee.pubkey(), false),
            AccountMeta::new(payer.pubkey(), false),
            AccountMeta::new_readonly(system_program::ID, false),
            AccountMeta::new_readonly(quoter_config_pubkey, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
            AccountMeta::new_readonly(event_cpi, false),
        ],
        data: ix_data,
    };

    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer.pubkey()), &[&payer], recent_blockhash);
    let result = banks_client.process_transaction(tx).await;
    assert!(result.is_err(), "Should fail with request_bytes_len overflow");
}

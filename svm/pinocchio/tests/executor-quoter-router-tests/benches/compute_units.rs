//! Compute unit benchmarks for executor-quoter-router using mollusk-svm.
//!
//! Run with: cargo bench -p executor-quoter-router-tests
//! Output: target/benches/executor_quoter_router_compute_units.md

use libsecp256k1::{Message, PublicKey, SecretKey};
use mollusk_svm::program::keyed_account_for_system_program;
use mollusk_svm::Mollusk;
use mollusk_svm_bencher::MolluskComputeUnitBencher;
use solana_sdk::{
    account::{AccountSharedData, WritableAccount},
    bpf_loader_upgradeable,
    instruction::{AccountMeta, Instruction},
    keccak,
    pubkey::Pubkey,
    rent::Rent,
    system_program,
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

// Account discriminators
const QUOTER_REGISTRATION_DISCRIMINATOR: u8 = 1;
const QUOTE_BODY_DISCRIMINATOR: u8 = 1;
const CHAIN_INFO_DISCRIMINATOR: u8 = 2;

// PDA seeds
const QUOTER_REGISTRATION_SEED: &[u8] = b"quoter_registration";
const QUOTER_CHAIN_INFO_SEED: &[u8] = b"chain_info";
const QUOTER_QUOTE_SEED: &[u8] = b"quote";

// Account sizes
const QUOTER_REGISTRATION_SIZE: usize = 54; // 1 + 1 + 20 + 32
const CHAIN_INFO_SIZE: usize = 8; // 1 + 1 + 2 + 1 + 1 + 1 + 1
const QUOTE_BODY_SIZE: usize = 40; // 1 + 1 + 2 + 4 + 8 + 8 + 8 + 8

// Instruction discriminators
const IX_UPDATE_QUOTER_CONTRACT: u8 = 0;
const IX_QUOTE_EXECUTION: u8 = 1;

// Wormhole chain ID for Solana
const SOLANA_CHAIN_ID: u16 = 1;

// Test destination chain
const DST_CHAIN_ID: u16 = 2;

/// Helper to derive quoter registration PDA
fn derive_quoter_registration_pda(quoter_address: &[u8; 20]) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[QUOTER_REGISTRATION_SEED, &quoter_address[..]],
        &ROUTER_PROGRAM_ID,
    )
}

fn derive_quoter_chain_info_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[QUOTER_CHAIN_INFO_SEED, &chain_id.to_le_bytes()],
        &QUOTER_PROGRAM_ID,
    )
}

fn derive_quoter_quote_body_pda(chain_id: u16) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[QUOTER_QUOTE_SEED, &chain_id.to_le_bytes()],
        &QUOTER_PROGRAM_ID,
    )
}

/// Secp256k1 quoter identity for testing.
struct QuoterIdentity {
    secret_key: SecretKey,
    eth_address: [u8; 20],
}

impl QuoterIdentity {
    /// Create a quoter identity from a fixed seed for deterministic tests.
    fn from_seed(seed: [u8; 32]) -> Self {
        let secret_key = SecretKey::parse(&seed).expect("valid seed");
        let public_key = PublicKey::from_secret_key(&secret_key);

        // Derive Ethereum address: keccak256(pubkey)[12:32]
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

    // Sign the body
    let (r, s, v) = quoter.sign(&body);

    // Build the full message
    let mut data = Vec::with_capacity(163);
    data.extend_from_slice(&body);
    data.extend_from_slice(&r);
    data.extend_from_slice(&s);
    data.push(v);

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

/// Build QuoteExecution instruction data (after 1-byte router discriminator).
/// Layout: quoter_address (20) + CPI data (discriminator + quote fields)
fn build_quote_execution_data(
    quoter_address: &[u8; 20],
    chain_id: u16,
    gas_limit: u128,
    msg_value: u128,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(IX_QUOTE_EXECUTION);
    // quoter_address (20 bytes)
    data.extend_from_slice(quoter_address);
    // CPI data: quoter RequestQuote discriminator (8 bytes)
    data.extend_from_slice(&[2, 0, 0, 0, 0, 0, 0, 0]);
    // dst_chain (2 bytes)
    data.extend_from_slice(&chain_id.to_le_bytes());
    // dst_addr (32 bytes)
    data.extend_from_slice(&[0xab; 32]);
    // refund_addr (32 bytes)
    data.extend_from_slice(&[0xcd; 32]);
    // request_bytes_len (4 bytes) = 0
    data.extend_from_slice(&0u32.to_le_bytes());
    // relay_instructions_len (4 bytes) = 33
    data.extend_from_slice(&33u32.to_le_bytes());
    // relay_instructions: type 1 (Gas)
    data.push(1);
    data.extend_from_slice(&gas_limit.to_be_bytes());
    data.extend_from_slice(&msg_value.to_be_bytes());
    data
}

/// Create a funded payer account
fn create_payer_account() -> AccountSharedData {
    AccountSharedData::new(1_000_000_000, 0, &system_program::ID)
}

/// Create a signer account
fn create_signer_account() -> AccountSharedData {
    AccountSharedData::new(0, 0, &system_program::ID)
}

/// Create a placeholder config account (config is not used, just required in account list)
fn create_config_account() -> AccountSharedData {
    AccountSharedData::new(0, 0, &system_program::ID)
}

/// Create an initialized QuoterRegistration account
fn create_quoter_registration_account(
    bump: u8,
    quoter_address: &[u8; 20],
    implementation_program_id: &Pubkey,
) -> AccountSharedData {
    let rent = Rent::default();
    let lamports = rent.minimum_balance(QUOTER_REGISTRATION_SIZE);
    let mut data = vec![0u8; QUOTER_REGISTRATION_SIZE];

    data[0] = QUOTER_REGISTRATION_DISCRIMINATOR;
    data[1] = bump;
    data[2..22].copy_from_slice(quoter_address);
    data[22..54].copy_from_slice(implementation_program_id.as_ref());

    let mut account =
        AccountSharedData::new(lamports, QUOTER_REGISTRATION_SIZE, &ROUTER_PROGRAM_ID);
    account.set_data_from_slice(&data);
    account
}

/// Create an initialized ChainInfo account for the quoter program
fn create_chain_info_account(chain_id: u16, bump: u8) -> AccountSharedData {
    let rent = Rent::default();
    let lamports = rent.minimum_balance(CHAIN_INFO_SIZE);
    let mut data = vec![0u8; CHAIN_INFO_SIZE];

    data[0] = CHAIN_INFO_DISCRIMINATOR;
    data[1] = bump;
    data[2..4].copy_from_slice(&chain_id.to_le_bytes());
    data[4] = 1;  // enabled
    data[5] = 15; // gas_price_decimals
    data[6] = 18; // native_decimals (ETH)

    let mut account = AccountSharedData::new(lamports, CHAIN_INFO_SIZE, &QUOTER_PROGRAM_ID);
    account.set_data_from_slice(&data);
    account
}

/// Create an initialized QuoteBody account for the quoter program
fn create_quote_body_account(chain_id: u16, bump: u8) -> AccountSharedData {
    let rent = Rent::default();
    let lamports = rent.minimum_balance(QUOTE_BODY_SIZE);
    let mut data = vec![0u8; QUOTE_BODY_SIZE];

    data[0] = QUOTE_BODY_DISCRIMINATOR;
    data[1] = bump;
    data[2..4].copy_from_slice(&chain_id.to_le_bytes());
    // _padding [4..8]
    data[8..16].copy_from_slice(&160_000_000u64.to_le_bytes());  // dst_price
    data[16..24].copy_from_slice(&2_650_000_000u64.to_le_bytes()); // src_price
    data[24..32].copy_from_slice(&399_146u64.to_le_bytes());     // dst_gas_price
    data[32..40].copy_from_slice(&100u64.to_le_bytes());         // base_fee

    let mut account = AccountSharedData::new(lamports, QUOTE_BODY_SIZE, &QUOTER_PROGRAM_ID);
    account.set_data_from_slice(&data);
    account
}

/// Create an executable program account for CPI
fn create_program_account() -> AccountSharedData {
    let mut account = AccountSharedData::new(1, 0, &bpf_loader_upgradeable::id());
    account.set_executable(true);
    account
}

fn main() {
    // Initialize Mollusk with router program + quoter for CPI
    let mut mollusk = Mollusk::new(&ROUTER_PROGRAM_ID, "executor_quoter_router");
    mollusk.add_program(&QUOTER_PROGRAM_ID, "executor_quoter");

    // Get the system program keyed account for CPI
    let system_program_account = keyed_account_for_system_program();

    // Set up accounts
    let payer = Pubkey::new_unique();
    let sender = Pubkey::new_unique();
    let config_pda = Pubkey::new_unique(); // placeholder, not used

    // Create a deterministic quoter identity
    let quoter = QuoterIdentity::from_seed([1u8; 32]);
    let (quoter_registration_pda, quoter_registration_bump) =
        derive_quoter_registration_pda(&quoter.eth_address);

    // Far future expiry time
    let expiry_time = u64::MAX;

    // =========================================================================
    // Benchmark: UpdateQuoterContract (create new)
    // =========================================================================
    let update_quoter_contract_create_ix = Instruction::new_with_bytes(
        ROUTER_PROGRAM_ID,
        &build_signed_update_quoter_contract_data(
            SOLANA_CHAIN_ID,
            &quoter,
            &QUOTER_PROGRAM_ID,
            &sender,
            expiry_time,
        ),
        vec![
            AccountMeta::new(payer, true),
            AccountMeta::new_readonly(sender, true),
            AccountMeta::new_readonly(config_pda, false),
            AccountMeta::new(quoter_registration_pda, false),
            AccountMeta::new_readonly(system_program::ID, false),
        ],
    );

    let update_quoter_contract_create_accounts = vec![
        (payer, create_payer_account()),
        (sender, create_signer_account()),
        (config_pda, create_config_account()),
        (
            quoter_registration_pda,
            AccountSharedData::new(0, 0, &system_program::ID),
        ),
        system_program_account.clone(),
    ];

    // Benchmark: UpdateQuoterContract (update existing)
    let update_quoter_contract_update_accounts = vec![
        (payer, create_payer_account()),
        (sender, create_signer_account()),
        (config_pda, create_config_account()),
        (
            quoter_registration_pda,
            create_quoter_registration_account(
                quoter_registration_bump,
                &quoter.eth_address,
                &QUOTER_PROGRAM_ID,
            ),
        ),
        system_program_account,
    ];

    // =========================================================================
    // Benchmark: QuoteExecution (CPI to quoter)
    // =========================================================================
    let quoter_config = Pubkey::new_unique();
    let (quoter_chain_info_pda, chain_info_bump) = derive_quoter_chain_info_pda(DST_CHAIN_ID);
    let (quoter_quote_body_pda, quote_body_bump) = derive_quoter_quote_body_pda(DST_CHAIN_ID);

    let quote_execution_ix = Instruction::new_with_bytes(
        ROUTER_PROGRAM_ID,
        &build_quote_execution_data(&quoter.eth_address, DST_CHAIN_ID, 250_000, 0),
        vec![
            AccountMeta::new_readonly(quoter_registration_pda, false),
            AccountMeta::new_readonly(QUOTER_PROGRAM_ID, false),
            AccountMeta::new_readonly(quoter_config, false),
            AccountMeta::new_readonly(quoter_chain_info_pda, false),
            AccountMeta::new_readonly(quoter_quote_body_pda, false),
        ],
    );

    let quote_execution_accounts = vec![
        (
            quoter_registration_pda,
            create_quoter_registration_account(
                quoter_registration_bump,
                &quoter.eth_address,
                &QUOTER_PROGRAM_ID,
            ),
        ),
        (QUOTER_PROGRAM_ID, create_program_account()),
        (quoter_config, create_config_account()),
        (
            quoter_chain_info_pda,
            create_chain_info_account(DST_CHAIN_ID, chain_info_bump),
        ),
        (
            quoter_quote_body_pda,
            create_quote_body_account(DST_CHAIN_ID, quote_body_bump),
        ),
    ];

    // Run benchmarks
    MolluskComputeUnitBencher::new(mollusk)
        .bench((
            "update_quoter_contract_create",
            &update_quoter_contract_create_ix,
            &update_quoter_contract_create_accounts,
        ))
        .bench((
            "update_quoter_contract_update",
            &update_quoter_contract_create_ix,
            &update_quoter_contract_update_accounts,
        ))
        .bench((
            "quote_execution_250k_gas",
            &quote_execution_ix,
            &quote_execution_accounts,
        ))
        .must_pass(true)
        .out_dir("target/benches")
        .execute();
}

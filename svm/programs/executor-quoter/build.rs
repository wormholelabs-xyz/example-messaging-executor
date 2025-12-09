use std::env;
use std::fs;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    // Parse QUOTER_UPDATER_PUBKEY env var (base58 pubkey string)
    // Falls back to a default test pubkey if not set
    let updater_pubkey = env::var("QUOTER_UPDATER_PUBKEY").expect("Updater pubkey must be set at build time");

    // Parse QUOTER_PAYEE_PUBKEY env var (base58 pubkey string)
    // Falls back to same as updater if not set
    let payee_pubkey = env::var("QUOTER_PAYEE_PUBKEY").unwrap_or_else(|_| updater_pubkey.clone());

    // Decode base58 to bytes
    let updater_bytes = bs58::decode(&updater_pubkey)
        .into_vec()
        .unwrap_or_else(|e| panic!("QUOTER_UPDATER_PUBKEY '{}' is not valid base58: {}", updater_pubkey, e));
    let payee_bytes = bs58::decode(&payee_pubkey)
        .into_vec()
        .unwrap_or_else(|e| panic!("QUOTER_PAYEE_PUBKEY '{}' is not valid base58: {}", payee_pubkey, e));

    assert_eq!(
        updater_bytes.len(),
        32,
        "QUOTER_UPDATER_PUBKEY must decode to 32 bytes, got {}",
        updater_bytes.len()
    );
    assert_eq!(
        payee_bytes.len(),
        32,
        "QUOTER_PAYEE_PUBKEY must decode to 32 bytes, got {}",
        payee_bytes.len()
    );

    // Write the byte arrays as includable Rust files
    fs::write(
        out_path.join("updater_address.rs"),
        format_byte_array(&updater_bytes),
    )
    .expect("Failed to write updater_address.rs");

    fs::write(
        out_path.join("payee_address.rs"),
        format_byte_array(&payee_bytes),
    )
    .expect("Failed to write payee_address.rs");

    // Rerun if env vars change
    println!("cargo:rerun-if-env-changed=QUOTER_UPDATER_PUBKEY");
    println!("cargo:rerun-if-env-changed=QUOTER_PAYEE_PUBKEY");
}

fn format_byte_array(bytes: &[u8]) -> String {
    let hex_bytes: Vec<String> = bytes.iter().map(|b| format!("0x{:02x}", b)).collect();
    format!("[{}]", hex_bytes.join(", "))
}

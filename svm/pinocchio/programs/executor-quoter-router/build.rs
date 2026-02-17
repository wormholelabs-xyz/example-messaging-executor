use std::env;
use std::fs;
use std::path::Path;

// Build-time configuration for executor-quoter-router.
//
// Optional env vars (all have Solana devnet defaults):
//   ROUTER_CHAIN_ID            - Wormhole chain ID for the deployment chain (u16).
//                                Default: 1 (Solana).
//   ROUTER_EXECUTOR_PROGRAM_ID - Base58-encoded Solana pubkey (32 bytes) of the
//                                executor program. Default: execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV.

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir);

    // Chain ID -- defaults to Solana (1).
    let chain_id: u16 = env::var("ROUTER_CHAIN_ID")
        .unwrap_or_else(|_| "1".to_string())
        .parse()
        .expect("ROUTER_CHAIN_ID must be a valid u16");

    fs::write(out_path.join("our_chain.rs"), format!("{chain_id}_u16")).unwrap();

    // Executor program ID -- defaults to devnet address.
    let executor_pubkey = env::var("ROUTER_EXECUTOR_PROGRAM_ID")
        .unwrap_or_else(|_| "execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV".to_string());

    let executor_bytes = bs58::decode(&executor_pubkey)
        .into_vec()
        .unwrap_or_else(|e| {
            panic!(
                "ROUTER_EXECUTOR_PROGRAM_ID '{}' is not valid base58: {}",
                executor_pubkey, e
            )
        });

    assert_eq!(
        executor_bytes.len(),
        32,
        "ROUTER_EXECUTOR_PROGRAM_ID must decode to 32 bytes, got {}",
        executor_bytes.len()
    );

    fs::write(
        out_path.join("executor_program_id.rs"),
        format_byte_array(&executor_bytes),
    )
    .unwrap();

    println!("cargo:rerun-if-env-changed=ROUTER_CHAIN_ID");
    println!("cargo:rerun-if-env-changed=ROUTER_EXECUTOR_PROGRAM_ID");
}

fn format_byte_array(bytes: &[u8]) -> String {
    let hex_bytes: Vec<String> = bytes.iter().map(|b| format!("0x{:02x}", b)).collect();
    format!("[{}]", hex_bytes.join(", "))
}

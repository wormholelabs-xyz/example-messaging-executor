# Pinocchio Programs

This directory contains Solana programs built with the Pinocchio framework for the executor quoter system.

## Directory Structure

- `programs/executor-quoter/` - Quoter program for price quotes
- `programs/executor-quoter-router/` - Router program for quoter registration and execution routing
- `programs/executor-quoter-tests/` - Integration tests for executor-quoter
- `programs/executor-quoter-router-tests/` - Integration tests for executor-quoter-router

## Prerequisites

- Solana CLI v1.18.17 or later
- A keypair file for the quoter updater address

Generate a test keypair if you don't have one:

```bash
mkdir -p ../test-keys
solana-keygen new --no-bip39-passphrase -o ../test-keys/quoter-updater.json
```

## Building

The Pinocchio programs must be built using `cargo build-sbf` before running tests.

### Build Programs

The `executor-quoter` program requires the `QUOTER_UPDATER_PUBKEY` environment variable to be set at build time. This is the public key authorized to update quotes.

```bash
cd svm/pinocchio

# Get the updater pubkey from your keypair
export QUOTER_UPDATER_PUBKEY=$(solana-keygen pubkey ../test-keys/quoter-updater.json)

# Build executor-quoter
cargo build-sbf --manifest-path programs/executor-quoter/Cargo.toml

# Build executor-quoter-router
cargo build-sbf --manifest-path programs/executor-quoter-router/Cargo.toml
```

### Build Anchor Executor (for router tests)

The router integration tests require the anchor executor program. Build it from the anchor directory:

```bash
cd ../anchor
cargo build-sbf --manifest-path programs/executor/Cargo.toml

# Copy to pinocchio deploy directory
cp target/deploy/executor.so ../pinocchio/target/deploy/
```

## Running Tests

Tests require several environment variables to be set:

- `QUOTER_UPDATER_PUBKEY` - Public key of the authorized updater
- `QUOTER_UPDATER_KEYPAIR_PATH` - Path to the updater keypair file
- `SBF_OUT_DIR` - Directory containing the compiled `.so` files

```bash
cd svm/pinocchio

export QUOTER_UPDATER_PUBKEY=$(solana-keygen pubkey ../test-keys/quoter-updater.json)
export QUOTER_UPDATER_KEYPAIR_PATH=$(pwd)/../test-keys/quoter-updater.json
export SBF_OUT_DIR=$(pwd)/target/deploy

# Run all tests
cargo test -p executor-quoter -p executor-quoter-tests -p executor-quoter-router-tests
```

Note: Do not use `cargo test --all` as it attempts to compile the BPF programs for native targets, which fails due to SBF-specific syscalls.

## Running Benchmarks

```bash
cd svm/pinocchio

# Benchmark executor-quoter
cargo bench -p executor-quoter-tests

# Benchmark executor-quoter-router
cargo bench -p executor-quoter-router-tests
```

## Notes

- The test crates use `solana-program-test` and [mollusk-svm](https://github.com/buffalojoec/mollusk) to load and execute the compiled `.so` files in a simulated SVM environment.
- Tests will fail if the `.so` files are not built first.
- The `QUOTER_UPDATER_PUBKEY` is baked into the program at compile time and cannot be changed without rebuilding.

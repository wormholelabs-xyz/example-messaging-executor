# Pinocchio Programs

Solana programs for the executor quoter system.

## Overview

- **executor-quoter-router** - Defines the quoter interface specification and routes CPI calls to registered quoter implementations. See [programs/executor-quoter-router/README.md](programs/executor-quoter-router/README.md).
- **executor-quoter** - Example quoter implementation. Integrators can use this as a reference or build their own. See [programs/executor-quoter/README.md](programs/executor-quoter/README.md).

These programs use the [Pinocchio](https://github.com/febo/pinocchio) framework, but quoter implementations are framework-agnostic. Any program adhering to the CPI interface defined by the router will work.

## Devnet Deployments

| Program                | Address                                        |
| ---------------------- | ---------------------------------------------- |
| executor-quoter        | `qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz`  |
| executor-quoter-router | `qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV` |

## Directory Structure

- `programs/executor-quoter/` - Example quoter implementation
- `programs/executor-quoter-router/` - Router program defining the quoter spec
- `tests/executor-quoter-tests/` - Integration tests and benchmarks for executor-quoter
- `tests/executor-quoter-router-tests/` - Integration tests and benchmarks for executor-quoter-router

## Prerequisites

- Solana CLI v1.18.17 or later

### Testing Prerequisites

Generate test keypairs before building or running tests:

```bash
mkdir -p ../test-keys
solana-keygen new --no-bip39-passphrase -o ../test-keys/quoter-updater.json
solana-keygen new --no-bip39-passphrase -o ../test-keys/quoter-payee.json
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

# Run unit tests (pure Rust math module)
cargo test -p executor-quoter

# Run integration tests (uses solana-program-test to simulate program execution)
cargo test -p executor-quoter-tests -p executor-quoter-router-tests -- --test-threads=1
```

Note: These tests use native `cargo test`, not `cargo test-sbf`. The unit tests are pure Rust without SBF dependencies. The integration tests use solana-program-test which loads the pre-built `.so` files and simulates program execution natively.

The `--test-threads=1` flag is required because `solana-program-test` can exhibit race conditions when multiple tests load BPF programs in parallel. Running tests sequentially avoids these issues.

## Running Benchmarks

```bash
cd svm/pinocchio

# Benchmark executor-quoter
cargo bench -p executor-quoter-tests

# Benchmark executor-quoter-router
cargo bench -p executor-quoter-router-tests
```

## Notes

- The test crates use `solana-program-test` to load and execute the compiled `.so` files in a simulated SVM environment. Benchmarks use [mollusk-svm](https://github.com/buffalojoec/mollusk) for compute unit measurements.
- Tests will fail if the `.so` files are not built first.
- The `QUOTER_UPDATER_PUBKEY` is baked into the program at compile time and cannot be changed without rebuilding.

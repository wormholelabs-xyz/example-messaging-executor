# Pinocchio Programs

Solana programs for the executor quoter system.

## Overview

- **executor-quoter-router** - Defines the quoter interface specification and routes CPI calls to registered quoter implementations. See [programs/executor-quoter-router/README.md](programs/executor-quoter-router/README.md).
- **executor-quoter** - Example quoter implementation. Integrators can use this as a reference or build their own. See [programs/executor-quoter/README.md](programs/executor-quoter/README.md).

These programs use the [Pinocchio](https://github.com/febo/pinocchio) framework, but quoter implementations are framework-agnostic. Any program adhering to the CPI interface defined by the router will work.

## Devnet Deployments

<!-- cspell:disable -->

| Program                | Address                                       |
| ---------------------- | --------------------------------------------- |
| executor-quoter        | `qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz` |
| executor-quoter-router | `qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV` |

<!-- cspell:enable -->

## Directory Structure

- `programs/executor-quoter/` - Example quoter implementation
- `programs/executor-quoter-router/` - Router program defining the quoter spec
- `tests/executor-quoter-tests/` - Integration tests and benchmarks for executor-quoter
- `tests/executor-quoter-router-tests/` - Integration tests and benchmarks for executor-quoter-router
- `modules/executor-requests/` - Shared request types (copy of `svm/modules/executor-requests/` for workspace-local builds)

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

The `executor-quoter` program requires build-time environment variables that are baked into the binary:

- `QUOTER_UPDATER_PUBKEY` (required) - Base58 Solana pubkey authorized to call `UpdateChainInfo` and `UpdateQuote`.
- `QUOTER_PAYEE_PUBKEY` (optional) - Base58 Solana pubkey used as the universal payee address for execution fees. Defaults to `QUOTER_UPDATER_PUBKEY` if unset.

```bash
cd svm/pinocchio

# Set build-time pubkeys from keypair files
export QUOTER_UPDATER_PUBKEY=$(solana-keygen pubkey ../test-keys/quoter-updater.json)
# Optional: set a separate payee (defaults to updater if omitted)
# export QUOTER_PAYEE_PUBKEY=$(solana-keygen pubkey ../test-keys/quoter-payee.json)

# Build executor-quoter
cargo build-sbf --manifest-path programs/executor-quoter/Cargo.toml

# Build executor-quoter-router (uses Solana defaults if env vars unset)
# Optional: override for non-Solana deployments (e.g. Fogo)
# export ROUTER_CHAIN_ID=10002
# export ROUTER_EXECUTOR_PROGRAM_ID=<base58 executor pubkey>
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

- `QUOTER_UPDATER_PUBKEY` - Base58 pubkey of the authorized updater (must match the value used at build time)
- `QUOTER_UPDATER_KEYPAIR_PATH` - Path to the updater keypair JSON file (used to sign test transactions)
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

## Verified Builds

Both programs support [solana-verify](https://github.com/Ellipsis-Labs/solana-verifiable-build) for on-chain verification on Solana explorers.

### Prerequisites

- Docker
- `cargo install solana-verify`

### How It Works

`solana-verify` builds inside a Docker container where no shell environment variables are set. The `.cargo/config.toml` in this workspace provides the required build-time env vars (pubkeys, chain ID) so Docker builds produce the correct binary without manual configuration.

Shell env vars take precedence over `config.toml` values (no `force` flag is set), so local and CI builds that export their own keys continue to work unchanged.

### Build and Verify

```bash
cd svm/pinocchio

# Build both programs inside Docker
solana-verify build --library-name executor_quoter_router
solana-verify build --library-name executor_quoter

# Compare local Docker build hash against on-chain program
solana-verify get-executable-hash target/deploy/executor_quoter_router.so
solana-verify get-program-hash -u devnet qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV

solana-verify get-executable-hash target/deploy/executor_quoter.so
solana-verify get-program-hash -u devnet qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz
```

### Verify From Repository

Once the programs are deployed from a Docker build and the commit is pushed:

<!-- cspell:disable -->

```bash
# executor-quoter-router
solana-verify verify-from-repo \
  -u https://api.devnet.solana.com \
  --program-id qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV \
  https://github.com/wormholelabs-xyz/example-messaging-executor \
  --mount-path svm/pinocchio \
  --library-name executor_quoter_router \
  --commit-hash <DEPLOYMENT_COMMIT>

# executor-quoter
solana-verify verify-from-repo \
  -u https://api.devnet.solana.com \
  --program-id qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz \
  https://github.com/wormholelabs-xyz/example-messaging-executor \
  --mount-path svm/pinocchio \
  --library-name executor_quoter \
  --commit-hash <DEPLOYMENT_COMMIT>
```

<!-- cspell:enable -->

### Updating Config Values

To deploy for a different environment (e.g. mainnet or Fogo), update the values in `.cargo/config.toml` to match the target deployment keys and chain ID.

### Keeping `modules/executor-requests` in Sync

`modules/executor-requests/` is a workspace-local copy of `svm/modules/executor-requests/` needed because `solana-verify` mounts only this workspace into Docker. The anchor workspace at `svm/anchor/` continues to reference the original. These two copies must stay in sync; any changes to the shared types should be applied to both.

## Notes

- The test crates use `solana-program-test` to load and execute the compiled `.so` files in a simulated SVM environment. Benchmarks use [mollusk-svm](https://github.com/buffalojoec/mollusk) for compute unit measurements.
- Tests will fail if the `.so` files are not built first.
- The `QUOTER_UPDATER_PUBKEY` is baked into the program at compile time and cannot be changed without rebuilding.

<!-- cspell:words Runbook -->

# Solana Program Verification Runbook

Verification steps for `executor-quoter` and `executor-quoter-router` using `solana-verify`.

## Prerequisites

- Docker running
- `cargo install solana-verify` (v0.4.12 or later recommended)
- Solana CLI with deploy authority keypair configured

## Docker Image Selection

`solana-verify` auto-selects the Docker image from the `solana-program` version in `Cargo.lock`. Because this workspace's test crates pull in `solana-sdk 1.18`, the auto-selected image uses an older toolchain that is incompatible with `pinocchio-system 0.4`.

Use `--base-image` to override with the Solana 2.3.13 image:

<!-- cspell:disable -->

```
--base-image "solanafoundation/solana-verifiable-build@sha256:f1f443a3b80fb688194849fbab66264eae7195ed85a7fe4b819cfa7f76f72d15"
```

<!-- cspell:enable -->

The image digest comes from `solana-verify`'s [image_config.rs](https://github.com/Ellipsis-Labs/solana-verifiable-build). If you upgrade `solana-verify`, check that this digest still maps to 2.3.13.

## 1. Update Git Dependencies

`executor-requests` is a git dependency pinned to a specific commit in `Cargo.lock`. Before building, update the pin to the latest commit:

```bash
make update-deps
# or: cargo update -p executor-requests
```

If `cargo update` bumps `Cargo.lock` to version 4, edit it back to version 3 (see Lockfile Version section below).

## 2. Docker Builds

Run from `svm/pinocchio/`:

```bash
# Recommended: uses Makefile which runs update-deps first
make build-verified

# Or build individually:
make build-router
make build-quoter
```

Build-time environment variables are passed via `--config` flags in the Makefile. For other environments, update the Makefile variables before building.

## 3. Compare Hashes

```bash
make verify-hashes
```

Or manually:

<!-- cspell:disable -->

```bash
# executor-quoter-router
solana-verify get-executable-hash target/deploy/executor_quoter_router.so
solana-verify get-program-hash -u devnet qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV

# executor-quoter
solana-verify get-executable-hash target/deploy/executor_quoter.so
solana-verify get-program-hash -u devnet qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz
```

<!-- cspell:enable -->

If hashes match, skip to step 5. If they differ, proceed to step 4.

## 4. Redeploy (if hashes differ)

Deploy the Docker-built `.so` files to devnet. PDA state (quoter registrations, chain info, quotes) is preserved across redeployment.

<!-- cspell:disable -->

```bash
solana program deploy target/deploy/executor_quoter_router.so \
  --program-id qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV \
  -u devnet

solana program deploy target/deploy/executor_quoter.so \
  --program-id qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz \
  -u devnet
```

<!-- cspell:enable -->

## 5. Push Commit and Verify From Repository

After the deploy commit is merged:

<!-- cspell:disable -->

```bash
BASE_IMAGE="solanafoundation/solana-verifiable-build@sha256:f1f443a3b80fb688194849fbab66264eae7195ed85a7fe4b819cfa7f76f72d15"

# executor-quoter-router
solana-verify verify-from-repo \
  -u https://api.devnet.solana.com \
  --program-id qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV \
  https://github.com/wormholelabs-xyz/example-messaging-executor \
  --mount-path svm/pinocchio \
  --library-name executor_quoter_router \
  --base-image "$BASE_IMAGE" \
  --commit-hash <DEPLOYMENT_COMMIT> \
  -- \
  --config 'env.ROUTER_CHAIN_ID="1"' \
  --config 'env.ROUTER_EXECUTOR_PROGRAM_ID="execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV"'

# executor-quoter
solana-verify verify-from-repo \
  -u https://api.devnet.solana.com \
  --program-id qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz \
  https://github.com/wormholelabs-xyz/example-messaging-executor \
  --mount-path svm/pinocchio \
  --library-name executor_quoter \
  --base-image "$BASE_IMAGE" \
  --commit-hash <DEPLOYMENT_COMMIT> \
  -- \
  --config 'env.QUOTER_UPDATER_PUBKEY="A6M3gQxPpLmFdA8tbPidM9fWp9wfmbebm2tSmAB2HTsY"' \
  --config 'env.QUOTER_PAYEE_PUBKEY="B4TMRgRPcyjiH5fBfNXssBrkorT6X3ystPNuJSoqrnFA"'
```

<!-- cspell:enable -->

## Build-Time Environment Variables

Both programs use `build.rs` scripts that read environment variables at compile time. These are passed to the Docker build via cargo `--config` flags in the Makefile:

<!-- cspell:disable -->

| Variable                     | Program                | Devnet Value                                   |
| ---------------------------- | ---------------------- | ---------------------------------------------- |
| `QUOTER_UPDATER_PUBKEY`      | executor-quoter        | `A6M3gQxPpLmFdA8tbPidM9fWp9wfmbebm2tSmAB2HTsY` |
| `QUOTER_PAYEE_PUBKEY`        | executor-quoter        | `B4TMRgRPcyjiH5fBfNXssBrkorT6X3ystPNuJSoqrnFA` |
| `ROUTER_CHAIN_ID`            | executor-quoter-router | `1`                                            |
| `ROUTER_EXECUTOR_PROGRAM_ID` | executor-quoter-router | `execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV`  |

<!-- cspell:enable -->

For local and CI builds, these are set as shell environment variables. For Docker builds, they are injected via `--config 'env.VAR="value"'`.

## Updating for Other Environments

To verify a mainnet or Fogo deployment, update the variable values in the `Makefile` to match the target deployment keys and chain ID before building.

## Lockfile Version

The `Cargo.lock` must remain at version 3. The Docker image's `cargo build-sbf` uses an older Cargo that does not support lockfile v4. If a local `cargo update` bumps the version to 4, manually edit it back to 3 before running Docker builds.

## Future Improvement

The `--base-image` override is needed because the test crates pull `solana-sdk 1.18` into `Cargo.lock`, causing `solana-verify` to auto-select an incompatible image. Upgrading test dependencies to `solana-sdk 2.x` would eliminate the need for `--base-image` and enable automatic explorer verification without manual intervention.

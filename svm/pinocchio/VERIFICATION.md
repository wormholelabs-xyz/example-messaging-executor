# Solana Program Verification Runbook

Verification steps for `executor-quoter` and `executor-quoter-router` using `solana-verify`.

## Prerequisites

- Docker running
- `cargo install solana-verify`
- Solana CLI with deploy authority keypair configured

## Docker Image Selection

`solana-verify` auto-selects the Docker image from the `solana-program` version in `Cargo.lock`. Because this workspace's test crates pull in `solana-sdk 1.18`, the auto-selected image uses an older toolchain (rustc 1.75) that is incompatible with `pinocchio-system 0.4`.

Use `--base-image` to override with the Solana 2.3.6 image (platform-tools v1.48, rustc 1.84):

```
--base-image "solanafoundation/solana-verifiable-build@sha256:ecfb304ab23f75c7a6c8440dc330cb96ce345b3014db859c165107f12ad59361"
```

The image digest comes from `solana-verify`'s [image_config.rs](https://github.com/Ellipsis-Labs/solana-verifiable-build). If you upgrade `solana-verify`, check that this digest still maps to 2.3.6.

## 1. Docker Builds

Run from `svm/pinocchio/`:

```bash
BASE_IMAGE="solanafoundation/solana-verifiable-build@sha256:ecfb304ab23f75c7a6c8440dc330cb96ce345b3014db859c165107f12ad59361"

solana-verify build --base-image "$BASE_IMAGE" --library-name executor_quoter_router
solana-verify build --base-image "$BASE_IMAGE" --library-name executor_quoter
```

## 2. Compare Hashes

Compare the Docker-built `.so` hashes against the on-chain programs:

```bash
# executor-quoter-router
solana-verify get-executable-hash target/deploy/executor_quoter_router.so
solana-verify get-program-hash -u devnet qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV

# executor-quoter
solana-verify get-executable-hash target/deploy/executor_quoter.so
solana-verify get-program-hash -u devnet qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz
```

If hashes match, skip to step 4. If they differ, proceed to step 3.

## 3. Redeploy (if hashes differ)

Deploy the Docker-built `.so` files to devnet. PDA state (quoter registrations, chain info, quotes) is preserved across redeployment.

```bash
solana program deploy target/deploy/executor_quoter_router.so \
  --program-id qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV \
  -u devnet

solana program deploy target/deploy/executor_quoter.so \
  --program-id qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz \
  -u devnet
```

## 4. Push Commit and Verify From Repository

After the deploy commit is merged:

```bash
BASE_IMAGE="solanafoundation/solana-verifiable-build@sha256:ecfb304ab23f75c7a6c8440dc330cb96ce345b3014db859c165107f12ad59361"

# executor-quoter-router
solana-verify verify-from-repo \
  -u https://api.devnet.solana.com \
  --program-id qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV \
  https://github.com/wormholelabs-xyz/example-messaging-executor \
  --mount-path svm/pinocchio \
  --library-name executor_quoter_router \
  --base-image "$BASE_IMAGE" \
  --commit-hash <DEPLOYMENT_COMMIT>

# executor-quoter
solana-verify verify-from-repo \
  -u https://api.devnet.solana.com \
  --program-id qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz \
  https://github.com/wormholelabs-xyz/example-messaging-executor \
  --mount-path svm/pinocchio \
  --library-name executor_quoter \
  --base-image "$BASE_IMAGE" \
  --commit-hash <DEPLOYMENT_COMMIT>
```

## How `.cargo/config.toml` Works

`solana-verify` builds inside Docker where no shell env vars exist. The `.cargo/config.toml` provides the required build-time values:

| Variable                     | Value                                        | Purpose                       |
| ---------------------------- | -------------------------------------------- | ----------------------------- |
| `QUOTER_UPDATER_PUBKEY`      | `A6M3gQxPpLmFdA8tbPidM9fWp9wfmbebm2tSmAB2HTsY` | Authorized updater for quoter |
| `QUOTER_PAYEE_PUBKEY`        | `B4TMRgRPcyjiH5fBfNXssBrkorT6X3ystPNuJSoqrnFA`  | Fee payee address             |
| `ROUTER_CHAIN_ID`            | `1`                                          | Wormhole chain ID (Solana)    |
| `ROUTER_EXECUTOR_PROGRAM_ID` | `execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV`   | Executor program              |

Shell env vars override these (no `force` flag), so CI and local builds are unaffected.

## Updating for Other Environments

To verify a mainnet or Fogo deployment, update the values in `.cargo/config.toml` to match the target deployment keys and chain ID before building.

## Lockfile Version

The `Cargo.lock` must remain at version 3. The Docker image's `cargo build-sbf` uses an older Cargo that does not support lockfile v4. If a local `cargo update` bumps the version to 4, manually edit it back to 3 before running Docker builds.

## Future Improvement

The `--base-image` override is needed because the test crates pull `solana-sdk 1.18` into `Cargo.lock`, causing `solana-verify` to auto-select an incompatible image. Upgrading test dependencies to `solana-sdk 2.x` would eliminate the need for `--base-image` and enable automatic explorer verification without manual intervention.

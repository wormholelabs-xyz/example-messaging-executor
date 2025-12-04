#!/bin/bash
# Compare Compute Units between Anchor, Native, and Pinocchio implementations
# Usage: ./scripts/compare-cu.sh [--skip-build]

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SVM_DIR="$(dirname "$SCRIPT_DIR")"

cd "$SVM_DIR"

# Program IDs
ANCHOR_PROGRAM_ID="2sDgzrUykBRKVzL3H4dT5wx7oiVAJg22kRVL7mhY1AqM"
PINOCCHIO_PROGRAM_ID="6yfXVhNgRKRk7YHFT8nTkVpFn5zXktbJddPUWK7jFAGX"
NATIVE_PROGRAM_ID="9CFzEuwodz3UhfZeDpBqpRJGpnYLbBcADMTUEmXvGu42"

SKIP_BUILD=false
if [[ "$1" == "--skip-build" ]]; then
  SKIP_BUILD=true
fi

# Build programs if needed
if [[ "$SKIP_BUILD" == "false" ]]; then
  echo "Building Pinocchio program..."
  cargo build-sbf --manifest-path programs/executor-quoter/Cargo.toml

  echo ""
  echo "Building Anchor program..."
  cargo build-sbf --manifest-path programs/executor-quoter-anchor/Cargo.toml

  echo ""
  echo "Building Native program..."
  cargo build-sbf --manifest-path programs/executor-quoter-native/Cargo.toml

  # Copy to target/deploy for consistency
  mkdir -p target/deploy
  cp target/sbpf-solana-solana/release/executor_quoter.so target/deploy/
  cp target/sbpf-solana-solana/release/executor_quoter_anchor.so target/deploy/
  cp target/sbpf-solana-solana/release/executor_quoter_native.so target/deploy/

  echo ""
else
  echo "Skipping build (--skip-build flag set)..."
  if [[ ! -f "target/deploy/executor_quoter_anchor.so" ]] || [[ ! -f "target/deploy/executor_quoter.so" ]] || [[ ! -f "target/deploy/executor_quoter_native.so" ]]; then
    echo "Error: Program binaries not found in target/deploy/"
    echo "Run without --skip-build to build them first."
    exit 1
  fi
fi

echo "Stopping any existing validator..."
pkill -f "solana-test-validator" 2>/dev/null || true
sleep 2

echo "Starting validator..."
solana-test-validator \
  --bpf-program "$ANCHOR_PROGRAM_ID" target/deploy/executor_quoter_anchor.so \
  --bpf-program "$PINOCCHIO_PROGRAM_ID" target/deploy/executor_quoter.so \
  --bpf-program "$NATIVE_PROGRAM_ID" target/deploy/executor_quoter_native.so \
  --reset \
  --quiet &

VALIDATOR_PID=$!

cleanup() {
  echo ""
  echo "Cleaning up..."
  kill $VALIDATOR_PID 2>/dev/null || true
  pkill -f "solana-test-validator" 2>/dev/null || true
}
trap cleanup EXIT

echo "Waiting for validator to start..."
sleep 10

echo "Running comparison tests..."
echo ""

OUTPUT=$(npx ts-mocha -p ./tsconfig.json -t 300000 tests/executor-quoter-comparison.ts 2>&1)

# Check if tests passed
if echo "$OUTPUT" | grep -q "passing"; then
  # Extract and display just the tables
  echo "$OUTPUT" | grep -E "^(=|\\|)"
  echo ""
  echo "Done!"
else
  echo "Tests failed:"
  echo "$OUTPUT"
  exit 1
fi

# SVM Programs

This directory contains Solana programs for the executor quoter system.

## Directory Structure

- `executor/` - Main executor program (Anchor-based)
- `executor-quoter/` - Quoter program for price quotes (pinocchio-based)
- `executor-quoter-router/` - Router program for quoter registration and execution routing (pinocchio-based)
- `executor-quoter-tests/` - Integration tests for executor-quoter
- `executor-quoter-router-tests/` - Integration tests for executor-quoter-router

## Building

The pinocchio-based programs (`executor-quoter` and `executor-quoter-router`) must be built using `cargo build-sbf` before running tests.

### Build Programs

```bash
# Build executor-quoter
cd svm/programs/executor-quoter
cargo build-sbf

# Build executor-quoter-router
cd svm/programs/executor-quoter-router
cargo build-sbf
```

### Run Tests

After building the `.so` files, run tests from the test crates:

```bash
# Run executor-quoter tests
cd svm/programs/executor-quoter-tests
cargo test

# Run executor-quoter-router tests
cd svm/programs/executor-quoter-router-tests
cargo test
```

### Run Benchmarks

```bash
# Benchmark executor-quoter
cd svm/programs/executor-quoter-tests
cargo bench

# Benchmark executor-quoter-router
cd svm/programs/executor-quoter-router-tests
cargo bench
```

## Notes

- The test crates use [mollusk-svm](https://github.com/buffalojoec/mollusk) to load and execute the compiled `.so` files in a simulated SVM environment.
- Tests will fail if the `.so` files are not built first.
- The main `executor` program uses Anchor and follows standard Anchor build/test workflows (see parent README).

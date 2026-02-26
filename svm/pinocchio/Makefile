# Verified build workflow for executor-quoter and executor-quoter-router.
#
# Both programs use build.rs scripts that read environment variables at compile
# time. The --config flags pass these values through to cargo inside the Docker
# container. For other environments (mainnet, Fogo), change the values below.
#
# executor-requests is a git dependency pinned in Cargo.lock. Before building
# for deployment, run `make update-deps` (or `make build-verified`, which
# includes it) to ensure the pinned commit matches the latest source.

BASE_IMAGE := solanafoundation/solana-verifiable-build@sha256:f1f443a3b80fb688194849fbab66264eae7195ed85a7fe4b819cfa7f76f72d15

# Devnet deployment values. Override these for other environments.
QUOTER_UPDATER_PUBKEY  := A6M3gQxPpLmFdA8tbPidM9fWp9wfmbebm2tSmAB2HTsY
QUOTER_PAYEE_PUBKEY    := B4TMRgRPcyjiH5fBfNXssBrkorT6X3ystPNuJSoqrnFA
ROUTER_CHAIN_ID        := 1
ROUTER_EXECUTOR_PROGRAM_ID := execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV

.PHONY: update-deps build-verified build-quoter build-router verify-hashes

# Pin executor-requests to the latest commit. Run before every deployment.
update-deps:
	cargo update -p executor-requests

# Build executor-quoter inside Docker.
build-quoter:
	solana-verify build \
		--base-image "$(BASE_IMAGE)" \
		--library-name executor_quoter \
		-- \
		--config 'env.QUOTER_UPDATER_PUBKEY="$(QUOTER_UPDATER_PUBKEY)"' \
		--config 'env.QUOTER_PAYEE_PUBKEY="$(QUOTER_PAYEE_PUBKEY)"'

# Build executor-quoter-router inside Docker.
build-router:
	solana-verify build \
		--base-image "$(BASE_IMAGE)" \
		--library-name executor_quoter_router \
		-- \
		--config 'env.ROUTER_CHAIN_ID="$(ROUTER_CHAIN_ID)"' \
		--config 'env.ROUTER_EXECUTOR_PROGRAM_ID="$(ROUTER_EXECUTOR_PROGRAM_ID)"'

# Full verified build: update deps, then Docker build both programs.
build-verified: update-deps build-router build-quoter

# Compare Docker-built hashes against devnet programs.
verify-hashes:
	@echo "executor-quoter-router:"
	@echo "  Local:    $$(solana-verify get-executable-hash target/deploy/executor_quoter_router.so)"
	@echo "  On-chain: $$(solana-verify get-program-hash -u devnet qtrrrV7W3E1jnX1145wXR6ZpthG19ur5xHC1n6PPhDV)"
	@echo ""
	@echo "executor-quoter:"
	@echo "  Local:    $$(solana-verify get-executable-hash target/deploy/executor_quoter.so)"
	@echo "  On-chain: $$(solana-verify get-program-hash -u devnet qtrxiqVAfVS61utwZLUi7UKugjCgFaNxBGyskmGingz)"

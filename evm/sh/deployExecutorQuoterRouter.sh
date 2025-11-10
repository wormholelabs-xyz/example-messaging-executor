#!/bin/bash

#
# This script deploys the Executor contract.
# Usage: RPC_URL= MNEMONIC= EXECUTOR= EVM_CHAIN_ID= ./sh/deployExecutorQuoterRouter.sh
#  anvil: EVM_CHAIN_ID=31337 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 ./sh/deployExecutorQuoterRouter.sh

if [ "${RPC_URL}X" == "X" ]; then
  RPC_URL=http://localhost:8545
fi

if [ "${MNEMONIC}X" == "X" ]; then
  MNEMONIC=0x4f3edf983ac636a65a842ce7c78d9aa706d3b113bce9c46f30d7d21715b23b1d
fi

if [ "${EVM_CHAIN_ID}X" == "X" ]; then
  EVM_CHAIN_ID=1337
fi

if [ -n "${CREATE2_ADDRESS}" ]; then
  CREATE2_FLAG="--create2-deployer ${CREATE2_ADDRESS}"
  echo "Using custom CREATE2 deployer at: ${CREATE2_ADDRESS}"
else
  CREATE2_FLAG=""
  echo "Using default CREATE2 deployer"
fi

forge script ./script/DeployExecutorQuoterRouter.s.sol:DeployExecutorQuoterRouter \
	--sig "run(address)" $EXECUTOR \
	--rpc-url "$RPC_URL" \
	--private-key "$MNEMONIC" \
  $CREATE2_FLAG \
	--broadcast ${FORGE_ARGS}

returnInfo=$(cat ./broadcast/DeployExecutorQuoterRouter.s.sol/$EVM_CHAIN_ID/run-latest.json)

DEPLOYED_ADDRESS=$(jq -r '.returns.deployedAddress.value' <<< "$returnInfo")
echo "Deployed ExecutorQuoterRouter address: $DEPLOYED_ADDRESS"

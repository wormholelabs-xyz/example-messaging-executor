#!/bin/bash

# Sample usage:
#   Sepolia: sh/verify.sh -r https://1rpc.io/sepolia 0x130e32c0B5B9c9823AAc21d7c53c753240Fd71e2
#   Base Sepolia: sh/verify.sh -r https://sepolia.base.org 0xb583a345f609bAbbB69ef96925BA13dea5152e34
#   Avalanche Fuji: sh/verify.sh -r https://rpc.ankr.com/avalanche_fuji 0xecabED02d566a6865Feb0C575375208d02D6bDf2

function usage() {
cat <<EOF >&2
Usage:

  $(basename "$0") [-h][-r rpc] <contract address> -- Verify that the deployed on-chain bytecode matches the local build artifact

  where:
      -h  show this help text
      -r  rpc url
EOF
exit 1
}

while getopts ':hn:r:c:' option; do
  case "$option" in
    h) usage
       ;;
    c) chain=$OPTARG
       ;;
    n) network=$OPTARG
       ;;
    r) rpc=$OPTARG
       ;;
    :) printf "missing argument for -%s\n" "$OPTARG" >&2
       usage
       ;;
   \?) printf "illegal option: -%s\n" "$OPTARG" >&2
       usage
       ;;
  esac
done
shift $((OPTIND - 1))
[ $# -ne 1 ] && usage

json_file=out/Executor.sol/Executor.json
contract_addr=$1

set -euo pipefail

# We'll write the bytecodes to temporary files
deployed=$(mktemp)
local=$(mktemp)

cat "$json_file" | jq -r .deployedBytecode | jq -r .object > "$local" 

ret=0
# Grab bytecode from the JSON RPC using the eth_getCode method.

curl "$rpc" \
  -X POST \
  -H "Content-Type: application/json" \
  --data "{\"method\":\"eth_getCode\",\"params\":[\"$contract_addr\",\"latest\"],\"id\":1,\"jsonrpc\":\"2.0\"}" --silent | jq -r .result > "$deployed" || ret=$?

if [ $ret -gt 0 ]; then
  printf "\033[0;31mFailed to query eth RPC '%s' while verifying %s on %s\033[0m\n" "$rpc" "$contract_addr" 
  exit 1
fi

echo "Deployed: " `cat $deployed`
echo "Local:    " `cat $local`

# hash, then see if they match up
hash1=$(sha256sum "$deployed" | cut -f1 -d' ')
hash2=$(sha256sum "$local" | cut -f1 -d' ')

if [ "$hash1" == "$hash2" ]; then
  printf "\033[0;32mDeployed bytecode of %s on %s matches %s\033[0m\n" "$contract_addr"  "$json_file";
  exit 0;
else
  printf "\033[0;31mDeployed bytecode of %s on %s doesn't match %s\033[0m\n" "$contract_addr"  "$json_file";
  echo "deployed hash:"
  echo "$hash1"
  echo "$json_file hash:"
  echo "$hash2"
  exit 1;
fi

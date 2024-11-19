// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

import {Executor, executorVersion} from "../src/Executor.sol";
import "forge-std/Script.sol";

// RequestExecution is a forge script to requestExecution on the Executor contract.

// forge script ./script/RequestExecution.s.sol:RequestExecution \
// 	--sig "run(address,uint16,bytes32,uint256,uint256,address,bytes,bytes)" $EXECUTOR_ADDRESS $DST_CHAIN $DST_ADDRESS $GAS_LIMIT $MSG_VALUE $REFUND_ADDRESS $SIGNED_QUOTE $REQUEST_BYTES \
// 	--rpc-url "$RPC_URL" \
// 	--private-key "$MNEMONIC" \
// 	--broadcast ${FORGE_ARGS}

// e.g.
// EVM_CHAIN_ID=31337 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 ./sh/deployExecutor.sh
// forge script ./script/RequestExecution.s.sol:RequestExecution --sig "run(address,uint16,bytes32,uint256,uint256,address,bytes,bytes)" 0x634fACff0663E8da9e9Eae4963d2F5006078b7BD 5 0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff 0 0 0xffffffffffffffffffffffffffffffffffffffff 455130313018f4dfe084527574ec1aa803e65478a451d83a000000000000000000000000ffffffffffffffffffffffffffffffffffffffff0002000500000000673ce4de000000000000000100000002540be4003dca4bee966e9d8e7962470dddcb049775debc1a9e50667876d3ae5fc4646f8a3382f6d5edd2d34b8515efeb1028e7b1b43c33cfacd8a1382544bc5bd9c40dee00 0x00 --rpc-url http://localhost:8545 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 --broadcast

contract RequestExecution is Script {
    function test() public {} // Exclude this from coverage report.

    function dryRun(
        address _executor,
        uint16 dstChain,
        bytes32 dstAddr,
        uint256 gasLimit,
        uint256 msgValue,
        address refundAddr,
        bytes calldata signedQuoteBytes,
        bytes calldata requestBytes
    ) public {
        _requestExecution(_executor, dstChain, dstAddr, gasLimit, msgValue, refundAddr, signedQuoteBytes, requestBytes);
    }

    function run(
        address _executor,
        uint16 dstChain,
        bytes32 dstAddr,
        uint256 gasLimit,
        uint256 msgValue,
        address refundAddr,
        bytes calldata signedQuoteBytes,
        bytes calldata requestBytes
    ) public {
        vm.startBroadcast();
        _requestExecution(_executor, dstChain, dstAddr, gasLimit, msgValue, refundAddr, signedQuoteBytes, requestBytes);
        vm.stopBroadcast();
    }

    function _requestExecution(
        address _executor,
        uint16 dstChain,
        bytes32 dstAddr,
        uint256 gasLimit,
        uint256 msgValue,
        address refundAddr,
        bytes calldata signedQuoteBytes,
        bytes calldata requestBytes
    ) internal {
        Executor executor = Executor(_executor);
        executor.requestExecution(dstChain, dstAddr, gasLimit, msgValue, refundAddr, signedQuoteBytes, requestBytes);
    }
}

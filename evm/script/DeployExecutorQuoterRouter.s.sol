// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

import {ExecutorQuoterRouter, EXECUTOR_QUOTER_ROUTER_VERSION_STR} from "../src/ExecutorQuoterRouter.sol";
import "forge-std/Script.sol";

// DeployExecutorQuoterRouter is a forge script to deploy the ExecutorQuoterRouter contract. Use ./sh/deployExecutorQuoterRouter.sh to invoke this.
// e.g. anvil
// EVM_CHAIN_ID=31337 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 ./sh/deployExecutorQuoterRouter.sh
// e.g. anvil --fork-url https://ethereum-rpc.publicnode.com
// EVM_CHAIN_ID=1 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 ./sh/deployExecutorQuoterRouter.sh
contract DeployExecutorQuoterRouter is Script {
    function test() public {} // Exclude this from coverage report.

    function dryRun(address executor) public {
        _deploy(executor);
    }

    function run(address executor) public returns (address deployedAddress) {
        vm.startBroadcast();
        (deployedAddress) = _deploy(executor);
        vm.stopBroadcast();
    }

    function _deploy(address executor) internal returns (address deployedAddress) {
        bytes32 salt = keccak256(abi.encodePacked(EXECUTOR_QUOTER_ROUTER_VERSION_STR));
        ExecutorQuoterRouter executorQuoterRouter = new ExecutorQuoterRouter{salt: salt}(executor);

        return (address(executorQuoterRouter));
    }
}

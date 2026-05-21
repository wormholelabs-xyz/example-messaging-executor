// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

import {ExecutorWithToken, executorVersion} from "../src/ExecutorWithToken.sol";
import "forge-std/Script.sol";

// DeployExecutorWithToken is a forge script to deploy the ExecutorWithToken contract. Use ./sh/deployExecutorWithToken.sh to invoke this.
// e.g. anvil
// EVM_CHAIN_ID=31337 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 EXECUTOR=<addr> ./sh/deployExecutorWithToken.sh
// e.g. anvil --fork-url https://ethereum-rpc.publicnode.com
// EVM_CHAIN_ID=1 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 EXECUTOR=<addr> ./sh/deployExecutorWithToken.sh
contract DeployExecutorWithToken is Script {
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
        bytes32 salt = keccak256(abi.encodePacked(executorVersion));
        ExecutorWithToken executorWithToken = new ExecutorWithToken{salt: salt}(executor);

        return (address(executorWithToken));
    }
}

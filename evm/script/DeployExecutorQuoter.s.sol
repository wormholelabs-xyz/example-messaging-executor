// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

import {ExecutorQuoter, EXECUTOR_QUOTER_VERSION_STR} from "../src/ExecutorQuoter.sol";
import "forge-std/Script.sol";

// DeployExecutorQuoter is a forge script to deploy the ExecutorQuoter contract. Use ./sh/deployExecutorQuoter.sh to invoke this.
// e.g. anvil
// EVM_CHAIN_ID=31337 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 ./sh/deployExecutorQuoter.sh
// e.g. anvil --fork-url https://ethereum-rpc.publicnode.com
// EVM_CHAIN_ID=1 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 ./sh/deployExecutorQuoter.sh
contract DeployExecutorQuoter is Script {
    function test() public {} // Exclude this from coverage report.

    function dryRun(address quoterAddress, address updaterAddress, uint8 srcTokenDecimals, bytes32 payeeAddress)
        public
    {
        _deploy(quoterAddress, updaterAddress, srcTokenDecimals, payeeAddress);
    }

    function run(address quoterAddress, address updaterAddress, uint8 srcTokenDecimals, bytes32 payeeAddress)
        public
        returns (address deployedAddress)
    {
        vm.startBroadcast();
        (deployedAddress) = _deploy(quoterAddress, updaterAddress, srcTokenDecimals, payeeAddress);
        vm.stopBroadcast();
    }

    function _deploy(address quoterAddress, address updaterAddress, uint8 srcTokenDecimals, bytes32 payeeAddress)
        internal
        returns (address deployedAddress)
    {
        bytes32 salt = keccak256(abi.encodePacked(EXECUTOR_QUOTER_VERSION_STR));
        ExecutorQuoter executorQuoter =
            new ExecutorQuoter{salt: salt}(quoterAddress, updaterAddress, srcTokenDecimals, payeeAddress);

        return (address(executorQuoter));
    }
}

// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

import {Executor, executorVersion} from "../src/Executor.sol";
import "forge-std/Script.sol";

// DeployExecutor is a forge script to deploy the Executor contract. Use ./sh/deployExecutor.sh to invoke this.
contract DeployExecutor is Script {
    function test() public {} // Exclude this from coverage report.

    function dryRun(uint16 ourChain) public {
        _deploy(ourChain);
    }

    function run(uint16 ourChain) public returns (address deployedAddress) {
        vm.startBroadcast();
        (deployedAddress) = _deploy(ourChain);
        vm.stopBroadcast();
    }

    function _deploy(uint16 ourChain) internal returns (address deployedAddress) {
        bytes32 salt = keccak256(abi.encodePacked(executorVersion));
        Executor executor = new Executor{salt: salt}(ourChain);

        return (address(executor));
    }
}

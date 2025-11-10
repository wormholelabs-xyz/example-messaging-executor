// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

interface IExecutorQuoter {
    function requestQuote(
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external view returns (bytes32, uint256);
}

// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

interface IExecutorQuoterRouter {
    event OnChainQuote(address implementation);
    event QuoterContractUpdate(address indexed quoterAddress, address implementation);

    function quoteExecution(
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        address quoterAddr,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external view returns (uint256);

    function requestExecution(
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        address quoterAddr,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external payable;
}

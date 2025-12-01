// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

interface IExecutorQuoter {
    /// This method is used by on- or off-chain services which need to determine the cost of a relay
    /// It only returns the required cost (msg.value)
    /// It is explicitly marked view
    function requestQuote(
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external view returns (uint256);

    /// This method is used by an ExecutorQuoterRouter during the execution flow
    /// It returns the required cost (msg.value) in addition to the payee and EQ02 quote body
    /// It is explicitly NOT marked view in order to allow the quoter the flexibility to emit events or update state
    function requestExecutionQuote(
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external returns (uint256, bytes32, bytes32);
}

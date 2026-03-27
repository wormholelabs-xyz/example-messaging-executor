// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

interface IExecutorWithToken {
    event ExecutionPayment(
        address indexed quoterAddress,
        uint256 amtPaid,
        address srcToken
    );

    function requestExecution(
        uint256 amount,
        address srcToken,
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        bytes calldata signedQuote,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external payable;
}

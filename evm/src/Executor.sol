// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import "./interfaces/IExecutor.sol";

string constant executorVersion = "Executor-0.0.1";

contract Executor is IExecutor {
    string public constant EXECUTOR_VERSION = executorVersion;

    uint16 public immutable ourChain;

    constructor(uint16 _ourChain) {
        ourChain = _ourChain;
    }

    error QuoteSrcChainMismatch(uint16 quoteSrcChain, uint16 requestSrcChain);
    error QuoteDstChainMismatch(uint16 quoteDstChain, uint16 requestDstChain);
    error QuoteExpired(uint64 expiryTime);
    error NotAnEvmAddress(bytes32);

    function requestExecution(
        uint16 dstChain,
        bytes32 dstAddr,
        uint256 gasLimit,
        uint256 msgValue,
        address refundAddr,
        SignedQuote calldata signedQuote,
        bytes calldata requestBytes
    ) public payable {
        if (signedQuote.srcChain != ourChain) {
            revert QuoteSrcChainMismatch(signedQuote.srcChain, ourChain);
        }
        if (signedQuote.dstChain != dstChain) {
            revert QuoteDstChainMismatch(signedQuote.dstChain, dstChain);
        }
        if (signedQuote.expiryTime <= block.timestamp) {
            revert QuoteExpired(signedQuote.expiryTime);
        }
        // Check if the higher 96 bits (left-most 12 bytes) are non-zero
        if (uint256(signedQuote.payeeAddress) >> 160 != 0) {
            revert NotAnEvmAddress(signedQuote.payeeAddress);
        }
        address payeeAddress = address(uint160(uint256(signedQuote.payeeAddress)));
        payable(payeeAddress).transfer(msg.value);
        emit RequestForExecution(
            signedQuote.quoterAddress,
            msg.value,
            dstChain,
            dstAddr,
            gasLimit,
            msgValue,
            refundAddr,
            signedQuote,
            requestBytes
        );
    }
}

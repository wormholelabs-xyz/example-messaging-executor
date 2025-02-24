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
        address refundAddr,
        bytes calldata signedQuoteBytes,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) public payable {
        {
            uint16 quoteSrcChain;
            uint16 quoteDstChain;
            uint64 expiryTime;
            assembly {
                quoteSrcChain := shr(240, calldataload(add(signedQuoteBytes.offset, 56)))
                quoteDstChain := shr(240, calldataload(add(signedQuoteBytes.offset, 58)))
                expiryTime := shr(192, calldataload(add(signedQuoteBytes.offset, 60)))
            }
            if (quoteSrcChain != ourChain) {
                revert QuoteSrcChainMismatch(quoteSrcChain, ourChain);
            }
            if (quoteDstChain != dstChain) {
                revert QuoteDstChainMismatch(quoteDstChain, dstChain);
            }
            if (expiryTime <= block.timestamp) {
                revert QuoteExpired(expiryTime);
            }
        }
        uint160 quoterAddress;
        bytes32 universalPayeeAddress;
        assembly {
            quoterAddress := shr(96, calldataload(add(signedQuoteBytes.offset, 4)))
            universalPayeeAddress := calldataload(add(signedQuoteBytes.offset, 24))
        }
        // Check if the higher 96 bits (left-most 12 bytes) are non-zero
        if (uint256(universalPayeeAddress) >> 160 != 0) {
            revert NotAnEvmAddress(universalPayeeAddress);
        }
        address payeeAddress = address(uint160(uint256(universalPayeeAddress)));
        payable(payeeAddress).transfer(msg.value);
        emit RequestForExecution(
            address(quoterAddress),
            msg.value,
            dstChain,
            dstAddr,
            refundAddr,
            signedQuoteBytes,
            requestBytes,
            relayInstructions
        );
    }
}

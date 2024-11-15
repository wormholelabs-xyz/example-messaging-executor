// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

interface IExecutor {
    struct Signature {
        bytes32 r;
        bytes32 s;
        uint8 v;
    }

    struct SignedQuote {
        bytes4 prefix;
        address quoterAddress;
        bytes32 payeeAddress;
        uint16 srcChain;
        uint16 dstChain;
        uint64 baseFee;
        uint64 conversionRate;
        uint64 expiryTime;
        Signature signature;
    }

    event RequestForExecution(
        address indexed quoterAddress,
        uint256 amtPaid,
        uint16 dstChain,
        bytes32 dstAddr,
        uint256 gasLimit,
        uint256 msgValue,
        address refundAddr,
        SignedQuote signedQuote,
        bytes requestBytes
    );

    function requestExecution(
        uint16 dstChain,
        bytes32 dstAddr,
        uint256 gasLimit,
        uint256 msgValue,
        address refundAddr,
        SignedQuote calldata signedQuote,
        bytes calldata requestBytes
    ) external payable;
}

// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

interface IExecutor {
    struct SignedQuoteHeader {
        bytes4 prefix;
        address quoterAddress;
        bytes32 payeeAddress;
        uint16 srcChain;
        uint16 dstChain;
        uint64 expiryTime;
    }

    event RequestForExecution(
        address indexed quoterAddress,
        uint256 amtPaid,
        uint16 dstChain,
        bytes32 dstAddr,
        uint256 gasLimit,
        uint256 msgValue,
        address refundAddr,
        bytes signedQuote,
        bytes requestBytes
    );

    /// @notice Payload length will not fit in a uint32.
    /// @dev Selector: 492f620d.
    error PayloadTooLarge();

    function requestExecution(
        uint16 dstChain,
        bytes32 dstAddr,
        uint256 gasLimit,
        uint256 msgValue,
        address refundAddr,
        bytes calldata signedQuote,
        bytes calldata requestBytes
    ) external payable;

    /// @notice Encodes a modular messaging request payload.
    /// @param sequence The sequence number returned by `endpoint.sendMessage`.
    /// @param payload The full payload, the keccak of which was sent to `endpoint.sendMessage`.
    /// @return bytes The encoded request.
    function makeMMRequest(uint64 sequence, bytes calldata payload) external view returns (bytes memory);

    /// @notice Encodes a version 1 VAA request payload.
    /// @param emitterChain The emitter chain from the VAA.
    /// @param emitterAddress The mitter address from the VAA.
    /// @param sequence The sequence number from the VAA.
    /// @return bytes The encoded request.
    function makeVAAV1Request(uint16 emitterChain, bytes32 emitterAddress, uint64 sequence)
        external
        pure
        returns (bytes memory);
}

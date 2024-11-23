// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

type UniversalAddress is bytes32;

library ExecutorMessages {
    bytes4 public constant REQ_MM = "ERM1";
    bytes4 public constant REQ_VAA_V1 = "ERV1";

    /// @notice Payload length will not fit in a uint32.
    /// @dev Selector: 492f620d.
    error PayloadTooLarge();

    /// @notice Encodes a modular messaging request payload.
    /// @param srcChain The source chain for the message (usually this chain).
    /// @param srcAddr The source address for the message.
    /// @param sequence The sequence number returned by `endpoint.sendMessage`.
    /// @param payload The full payload, the keccak of which was sent to `endpoint.sendMessage`.
    /// @return bytes The encoded request.
    function makeMMRequest(uint16 srcChain, address srcAddr, uint64 sequence, bytes calldata payload)
        public
        pure
        returns (bytes memory)
    {
        if (payload.length > type(uint32).max) {
            revert PayloadTooLarge();
        }
        return abi.encodePacked(
            REQ_MM, srcChain, bytes32(uint256(uint160(srcAddr))), sequence, uint32(payload.length), payload
        );
    }

    /// @notice Encodes a version 1 VAA request payload.
    /// @param emitterChain The emitter chain from the VAA.
    /// @param emitterAddress The mitter address from the VAA.
    /// @param sequence The sequence number from the VAA.
    /// @return bytes The encoded request.
    function makeVAAV1Request(uint16 emitterChain, bytes32 emitterAddress, uint64 sequence)
        public
        pure
        returns (bytes memory)
    {
        return abi.encodePacked(REQ_VAA_V1, emitterChain, emitterAddress, sequence);
    }
}

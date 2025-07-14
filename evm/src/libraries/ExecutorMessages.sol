// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

library ExecutorMessages {
    bytes4 private constant REQ_VAA_V1 = "ERV1";
    bytes4 private constant REQ_NTT_V1 = "ERN1";
    bytes4 private constant REQ_CCTP_V1 = "ERC1";
    bytes4 private constant REQ_CCTP_V2 = "ERC2";

    /// @notice Payload length will not fit in a uint32.
    /// @dev Selector: 492f620d.
    error PayloadTooLarge();

    /// @notice Encodes a version 1 VAA request payload.
    /// @param emitterChain The emitter chain from the VAA.
    /// @param emitterAddress The emitter address from the VAA.
    /// @param sequence The sequence number from the VAA.
    /// @return bytes The encoded request.
    function makeVAAv1Request(uint16 emitterChain, bytes32 emitterAddress, uint64 sequence)
        internal
        pure
        returns (bytes memory)
    {
        return abi.encodePacked(REQ_VAA_V1, emitterChain, emitterAddress, sequence);
    }

    /// @notice Encodes a version 1 NTT request payload.
    /// @param srcChain The source chain for the NTT transfer.
    /// @param srcManager The source manager for the NTT transfer.
    /// @param messageId The manager message id for the NTT transfer.
    /// @return bytes The encoded request.
    function makeNTTv1Request(uint16 srcChain, bytes32 srcManager, bytes32 messageId)
        internal
        pure
        returns (bytes memory)
    {
        return abi.encodePacked(REQ_NTT_V1, srcChain, srcManager, messageId);
    }

    /// @notice Encodes a version 1 CCTP request payload.
    /// @param sourceDomain The source chain for the CCTP transfer.
    /// @param nonce The nonce of the CCTP transfer.
    /// @return bytes The encoded request.
    function makeCCTPv1Request(uint32 sourceDomain, uint64 nonce) internal pure returns (bytes memory) {
        return abi.encodePacked(REQ_CCTP_V1, sourceDomain, nonce);
    }

    /// @notice Encodes a version 2 CCTP request payload.
    ///         This request currently assumes the Executor will auto detect the event off chain.
    ///         That may change in the future, in which case this interface would change.
    /// @return bytes The encoded request.
    function makeCCTPv2Request() internal pure returns (bytes memory) {
        return abi.encodePacked(REQ_CCTP_V2, uint8(1));
    }
}

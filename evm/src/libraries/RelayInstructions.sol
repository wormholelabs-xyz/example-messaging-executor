// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

library RelayInstructions {
    uint8 private constant RECV_INST_TYPE_GAS = uint8(1);
    uint8 private constant RECV_INST_TYPE_DROP_OFF = uint8(2);

    /// @notice Encodes the gas parameters for the relayer.
    /// @dev This instruction may be specified more than once. If so, the relayer should sum the values.
    /// @param gasLimit The gas limit passed to the relayer.
    /// @param msgVal The additional message value passed to the relayer. This may be zero.
    /// @return bytes The encoded instruction bytes.
    function encodeGas(uint128 gasLimit, uint128 msgVal) internal pure returns (bytes memory) {
        return abi.encodePacked(RECV_INST_TYPE_GAS, gasLimit, msgVal);
    }

    /// @notice Encodes the gas drop off parameters for the relayer.
    /// @param dropOff The amount of gas to be dropped off.
    /// @param recipient The recipient of the drop off.
    /// @return bytes The encoded instruction bytes.
    function encodeGasDropOffInstructions(uint128 dropOff, bytes32 recipient) internal pure returns (bytes memory) {
        return abi.encodePacked(RECV_INST_TYPE_DROP_OFF, dropOff, recipient);
    }
}

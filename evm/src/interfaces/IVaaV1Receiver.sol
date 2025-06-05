// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.19;

/// @notice Any contract that wishes to receive V1 VAAs from the executor needs to implement `IVaaV1Receiver`.
interface IVaaV1Receiver {
    /// @notice Receive an attested message from the executor relayer.
    /// @param msg The attested message payload.
    function executeVAAv1(bytes memory msg) external payable;
}

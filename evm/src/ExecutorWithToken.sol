// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import "./interfaces/IExecutor.sol";
import "./interfaces/IExecutorWithToken.sol";
import {SafeTransferLib} from "solady/utils/SafeTransferLib.sol";

string constant executorVersion = "Executor-With-Token-0.0.1";

contract ExecutorWithToken is IExecutorWithToken {
    string public constant EXECUTOR_WITH_TOKEN_VERSION = executorVersion;

    IExecutor public immutable EXECUTOR;

    error NotAnEvmAddress(bytes32);

    constructor(address _executor) {
        EXECUTOR = IExecutor(_executor);
    }

    function requestExecution(
        uint256 amount,
        address srcToken,
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        bytes calldata signedQuote,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external payable {
        uint160 quoterAddress;
        bytes32 universalPayeeAddress;
        assembly {
            quoterAddress := shr(96, calldataload(add(signedQuote.offset, 4)))
            universalPayeeAddress := calldataload(add(signedQuote.offset, 24))
        }
        if (uint256(universalPayeeAddress) >> 160 != 0) {
            revert NotAnEvmAddress(universalPayeeAddress);
        }
        address payeeAddress = address(uint160(uint256(universalPayeeAddress)));
        SafeTransferLib.safeTransferFrom(srcToken, msg.sender, payeeAddress, amount);
        EXECUTOR.requestExecution{value: msg.value}(
            dstChain, dstAddr, refundAddr, signedQuote, requestBytes, relayInstructions
        );
        emit ExecutionPayment(address(quoterAddress), amount, srcToken);
    }
}

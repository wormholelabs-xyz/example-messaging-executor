// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import "./interfaces/IExecutor.sol";
import "./interfaces/IExecutorQuoter.sol";
import "./interfaces/IExecutorQuoterRouter.sol";

string constant EXECUTOR_QUOTER_ROUTER_VERSION_STR = "Executor-Quote-Router-0.0.1";

contract ExecutorQuoterRouter is IExecutorQuoterRouter {
    string public constant EXECUTOR_QUOTER_ROUTER_VERSION = EXECUTOR_QUOTER_ROUTER_VERSION_STR;
    bytes4 private constant QUOTE_PREFIX = "EQ02";
    bytes4 private constant GOVERNANCE_PREFIX = "EG01";
    uint64 private constant EXPIRY_TIME = type(uint64).max;

    IExecutor public immutable EXECUTOR;
    uint16 public immutable OUR_CHAIN;

    mapping(address => IExecutorQuoter) public quoterContract;

    /// @notice Error when the payment is less than required.
    /// @dev Selector 0xf3ebc384.
    /// @param provided The msg.value.
    /// @param expected The required payment from the quoter.
    error Underpaid(uint256 provided, uint256 expected);
    /// @notice Error when the refund to the sender fails.
    /// @dev Selector 0x2645bdc2.
    /// @param refundAddr The refund address.
    error RefundFailed(address refundAddr);
    error ChainIdMismatch(uint16 govChain, uint16 ourChain);
    error InvalidSender();
    error InvalidSignature();
    error GovernanceExpired(uint64 expiryTime);
    error NotAnEvmAddress(bytes32);

    constructor(address _executor) {
        EXECUTOR = IExecutor(_executor);
        OUR_CHAIN = EXECUTOR.ourChain();
    }

    function updateQuoterContract(bytes calldata gov) external {
        bytes4 prefix;
        uint16 chainId;
        uint160 quoter;
        address quoterAddr;
        bytes32 universalContractAddress;
        bytes32 universalSenderAddress;
        uint64 expiryTime;
        bytes32 r;
        bytes32 s;
        uint8 v;
        assembly {
            prefix := calldataload(gov.offset)
            chainId := shr(240, calldataload(add(gov.offset, 4)))
            quoter := shr(96, calldataload(add(gov.offset, 6)))
            universalContractAddress := calldataload(add(gov.offset, 26))
            universalSenderAddress := calldataload(add(gov.offset, 58))
            expiryTime := shr(192, calldataload(add(gov.offset, 90)))
            r := calldataload(add(gov.offset, 98))
            s := calldataload(add(gov.offset, 130))
            v := shr(248, calldataload(add(gov.offset, 162)))
        }
        if (chainId != OUR_CHAIN) {
            revert ChainIdMismatch(chainId, OUR_CHAIN);
        }
        // Check if the higher 96 bits (left-most 12 bytes) are non-zero
        if (uint256(universalContractAddress) >> 160 != 0) {
            revert NotAnEvmAddress(universalContractAddress);
        }
        // Check if the higher 96 bits (left-most 12 bytes) are non-zero
        if (uint256(universalSenderAddress) >> 160 != 0) {
            revert NotAnEvmAddress(universalSenderAddress);
        }
        address senderAddress = address(uint160(uint256(universalSenderAddress)));
        if (msg.sender != senderAddress) {
            revert InvalidSender();
        }
        if (expiryTime <= block.timestamp) {
            revert GovernanceExpired(expiryTime);
        }
        quoterAddr = address(quoter);
        bytes32 hash = keccak256(gov[0:98]);
        address signer = ecrecover(hash, v, r, s);
        if (signer == address(0)) {
            revert InvalidSignature();
        }
        if (signer != quoterAddr) {
            revert InvalidSignature();
        }
        address contractAddress = address(uint160(uint256(universalContractAddress)));
        quoterContract[quoterAddr] = IExecutorQuoter(contractAddress);
        emit QuoterContractUpdate(quoterAddr, contractAddress);
    }

    function quoteExecution(
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        address quoterAddr,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external view returns (uint256 requiredPayment) {
        requiredPayment =
            quoterContract[quoterAddr].requestQuote(dstChain, dstAddr, refundAddr, requestBytes, relayInstructions);
    }

    function requestExecution(
        uint16 dstChain,
        bytes32 dstAddr,
        address refundAddr,
        address quoterAddr,
        bytes calldata requestBytes,
        bytes calldata relayInstructions
    ) external payable {
        IExecutorQuoter implementation = quoterContract[quoterAddr];
        (uint256 requiredPayment, bytes32 payeeAddress, bytes32 quoteBody) =
            implementation.requestExecutionQuote(dstChain, dstAddr, refundAddr, requestBytes, relayInstructions);
        if (msg.value < requiredPayment) {
            revert Underpaid(msg.value, requiredPayment);
        }
        if (msg.value > requiredPayment) {
            (bool refundSuccessful,) = payable(refundAddr).call{value: msg.value - requiredPayment}("");
            if (!refundSuccessful) {
                revert RefundFailed(refundAddr);
            }
        }
        EXECUTOR.requestExecution{value: requiredPayment}(
            dstChain,
            dstAddr,
            refundAddr,
            abi.encodePacked(QUOTE_PREFIX, quoterAddr, payeeAddress, OUR_CHAIN, dstChain, EXPIRY_TIME, quoteBody),
            requestBytes,
            relayInstructions
        );
        // this must emit a message in this function in order to verify off-chain that this contract generated the quote
        // the implementation is the only data available in this context that is not available from the executor event
        emit OnChainQuote(address(implementation));
    }
}

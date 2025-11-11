// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import "./interfaces/IExecutorQuoter.sol";

string constant executorQuoterVersion = "Executor-Quoter-0.0.1";

contract ExecutorQuoter is IExecutorQuoter {
    string public constant EXECUTOR_QUOTER_VERSION = executorQuoterVersion;
    uint8 private constant QUOTE_DECIMALS = 10;
    uint8 private constant DECIMAL_RESOLUTION = 18;

    address public immutable quoterAddress;
    address public immutable updaterAddress;
    uint8 public immutable srcTokenDecimals;
    bytes32 public immutable payeeAddress;

    /// This is the same as an EQ01 quote body
    /// It fits into a single bytes32 storage slot
    struct OnChainQuoteBody {
        /// The base fee, in sourceChain native currency, required by the quoter to perform an execution on the destination chain
        uint64 baseFee;
        /// The current gas price on the destination chain
        uint64 dstGasPrice;
        /// The USD price, in 10^10, of the sourceChain native currency
        uint64 srcPrice;
        /// The USD price, in 10^10, of the destinationChain native currency
        uint64 dstPrice;
    }

    struct ChainInfo {
        bool enabled;
        uint8 gasPriceDecimals;
        uint8 nativeDecimals;
    }

    struct Update {
        uint16 chainId;
        bytes32 update;
    }

    mapping(uint16 => OnChainQuoteBody) public quoteByDstChain;
    mapping(uint16 => ChainInfo) public chainInfos;

    /// @dev Selector 0x40788bb5.
    error InvalidUpdater(address sender, address expected);
    /// @dev Selector 0x4dc2c273.
    error ChainDisabled(uint16 chainId);
    /// @dev Selector 0x0d2e6713.
    error UnsupportedInstruction(uint8 ixType);
    /// @dev Selector 0x3a5a1720.
    error MoreThanOneDropOff();

    modifier onlyUpdater() {
        if (msg.sender != updaterAddress) {
            revert InvalidUpdater(msg.sender, updaterAddress);
        }
        _;
    }

    constructor(address _quoterAddress, address _updaterAddress, uint8 _srcTokenDecimals, bytes32 _payeeAddress) {
        quoterAddress = _quoterAddress;
        updaterAddress = _updaterAddress;
        srcTokenDecimals = _srcTokenDecimals;
        payeeAddress = _payeeAddress;
    }

    function _batchUpdate(Update[] calldata updates, uint256 mappingSlot) private {
        assembly {
            let len := updates.length
            let baseOffset := updates.offset

            for { let i := 0 } lt(i, len) { i := add(i, 1) } {
                // Load update directly from calldata
                let updatePtr := add(baseOffset, mul(i, 0x40))
                let chainId := calldataload(updatePtr)
                let newValue := calldataload(add(updatePtr, 0x20))

                // Calculate storage slot for mapping[chainId]
                mstore(0x00, chainId)
                mstore(0x20, mappingSlot)
                let slot := keccak256(0x00, 0x40)
                sstore(slot, newValue)
            }
        }
    }

    function chainInfoUpdate(Update[] calldata updates) external onlyUpdater {
        uint256 slot;
        assembly {
            slot := chainInfos.slot
        }
        _batchUpdate(updates, slot);
    }

    function quoteUpdate(Update[] calldata updates) external onlyUpdater {
        uint256 slot;
        assembly {
            slot := quoteByDstChain.slot
        }
        _batchUpdate(updates, slot);
    }

    function normalize(uint256 amount, uint8 from, uint8 to) private pure returns (uint256) {
        if (from > to) {
            return amount / 10 ** uint256(from - to);
        } else if (from < to) {
            return amount * 10 ** uint256(to - from);
        }
        return amount;
    }

    function mul(uint256 a, uint256 b, uint8 decimals) private pure returns (uint256) {
        return (a * b) / 10 ** uint256(decimals);
    }

    function div(uint256 a, uint256 b, uint8 decimals) private pure returns (uint256) {
        return (a * 10 ** uint256(decimals)) / b;
    }

    /// Calculates the total gas limit and total message value from a set of relay instructions.
    /// Each relay instruction can be either a `GasInstruction` or a `GasDropOffInstruction`.
    /// - `GasInstruction` contributes to both `gasLimit` and `msgValue`.
    /// - `GasDropOffInstruction` contributes only to `msgValue`.
    /// Throws If an unsupported instruction type is encountered.
    function totalGasLimitAndMsgValue(bytes calldata relayInstructions)
        private
        pure
        returns (uint256 gasLimit, uint256 msgValue)
    {
        uint256 offset = 0;
        uint8 ixType;
        uint128 ixGasLimit;
        uint128 ixMsgValue;
        bool hasDropOff = false;
        uint256 relayInstructionsLength = relayInstructions.length;
        while (offset < relayInstructionsLength) {
            assembly {
                ixType := shr(248, calldataload(add(relayInstructions.offset, offset)))
                offset := add(offset, 1)
            }
            if (ixType == 1) {
                assembly {
                    ixGasLimit := shr(128, calldataload(add(relayInstructions.offset, offset)))
                    offset := add(offset, 16)
                    ixMsgValue := shr(128, calldataload(add(relayInstructions.offset, offset)))
                    offset := add(offset, 16)
                }
                gasLimit = gasLimit + ixGasLimit;
                msgValue = msgValue + ixMsgValue;
            } else if (ixType == 2) {
                if (hasDropOff) {
                    revert MoreThanOneDropOff();
                }
                hasDropOff = true;
                assembly {
                    ixMsgValue := shr(128, calldataload(add(relayInstructions.offset, offset)))
                    offset := add(offset, 48)
                }
                msgValue = msgValue + ixMsgValue;
            } else {
                revert UnsupportedInstruction(ixType);
            }
        }
    }

    function estimateQuote(
        OnChainQuoteBody storage quote,
        ChainInfo storage dstChainInfo,
        uint256 gasLimit,
        uint256 msgValue
    ) private view returns (uint256) {
        uint256 srcChainValueForBaseFee = normalize(quote.baseFee, QUOTE_DECIMALS, srcTokenDecimals);

        uint256 nSrcPrice = normalize(quote.srcPrice, QUOTE_DECIMALS, DECIMAL_RESOLUTION);
        uint256 nDstPrice = normalize(quote.dstPrice, QUOTE_DECIMALS, DECIMAL_RESOLUTION);
        uint256 scaledConversion = div(nDstPrice, nSrcPrice, DECIMAL_RESOLUTION);

        uint256 nGasLimitCost =
            normalize(gasLimit * quote.dstGasPrice, dstChainInfo.gasPriceDecimals, DECIMAL_RESOLUTION);
        uint256 srcChainValueForGasLimit =
            normalize(mul(nGasLimitCost, scaledConversion, DECIMAL_RESOLUTION), DECIMAL_RESOLUTION, srcTokenDecimals);

        uint256 nMsgValue = normalize(msgValue, dstChainInfo.nativeDecimals, DECIMAL_RESOLUTION);
        uint256 srcChainValueForMsgValue =
            normalize(mul(nMsgValue, scaledConversion, DECIMAL_RESOLUTION), DECIMAL_RESOLUTION, srcTokenDecimals);
        return srcChainValueForBaseFee + srcChainValueForGasLimit + srcChainValueForMsgValue;
    }

    function requestQuote(
        uint16 dstChain,
        bytes32, //dstAddr,
        address, //refundAddr,
        bytes calldata, //requestBytes,
        bytes calldata relayInstructions
    ) external view returns (bytes32, uint256) {
        ChainInfo storage dstChainInfo = chainInfos[dstChain];
        if (!dstChainInfo.enabled) {
            revert ChainDisabled(dstChain);
        }
        OnChainQuoteBody storage quote = quoteByDstChain[dstChain];
        (uint256 gasLimit, uint256 msgValue) = totalGasLimitAndMsgValue(relayInstructions);
        // NOTE: this does not include any maxGasLimit or maxMsgValue checks
        uint256 requiredPayment = estimateQuote(quote, dstChainInfo, gasLimit, msgValue);

        return (payeeAddress, requiredPayment);
    }
}

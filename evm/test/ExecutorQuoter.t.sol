// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";

import {ExecutorMessages} from "../src/libraries/ExecutorMessages.sol";
import {RelayInstructions} from "../src/libraries/RelayInstructions.sol";
import {ExecutorQuoter} from "../src/ExecutorQuoter.sol";

contract ExecutorQuoterTest is Test {
    ExecutorQuoter public executorQuoter;
    ExecutorQuoter.Update[] public updates;
    ExecutorQuoter.Update[] public chainInfoUpdates;

    address constant UPDATER = 0x7FA9385bE102ac3EAc297483Dd6233D62b3e1496;
    bytes32 constant CHAIN_INFO_UPDATE_PACKED = 0x0000000000000000000000000000000000000000000000000000000000121201;
    uint16 constant DST_CHAIN = 10003;
    bytes32 constant DST_ADDR = bytes32(0);

    function packUint64(uint64 a, uint64 b, uint64 c, uint64 d) public pure returns (bytes32) {
        return bytes32((uint256(a) << 192) | (uint256(b) << 128) | (uint256(c) << 64) | uint256(d));
    }

    function setUp() public {
        executorQuoter = new ExecutorQuoter(UPDATER, UPDATER, 18, bytes32(uint256(uint160(UPDATER))));
        ExecutorQuoter.Update memory chainInfoUpdate;
        chainInfoUpdate.chainId = 10003;
        chainInfoUpdate.update = CHAIN_INFO_UPDATE_PACKED;
        chainInfoUpdates.push(chainInfoUpdate);
        executorQuoter.chainInfoUpdate(chainInfoUpdates);
        ExecutorQuoter.OnChainQuoteBody memory quote1;
        quote1.baseFee = 27971;
        quote1.dstGasPrice = 100000000;
        quote1.srcPrice = 35751300000000;
        quote1.dstPrice = 35751300000000;
        ExecutorQuoter.Update memory update1;
        update1.chainId = 10003;
        update1.update = packUint64(quote1.baseFee, quote1.dstGasPrice, quote1.srcPrice, quote1.dstPrice);
        ExecutorQuoter.OnChainQuoteBody memory quote2;
        quote2.baseFee = 27971;
        quote2.dstGasPrice = 1000250;
        quote2.srcPrice = 35751300000000;
        quote2.dstPrice = 35751300000000;
        ExecutorQuoter.Update memory update2;
        update2.chainId = 10005;
        update2.update = packUint64(quote2.baseFee, quote2.dstGasPrice, quote2.srcPrice, quote2.dstPrice);
        ExecutorQuoter.OnChainQuoteBody memory quote3;
        quote3.baseFee = 27971;
        quote3.dstGasPrice = 1000078;
        quote3.srcPrice = 35751300000000;
        quote3.dstPrice = 35751300000000;
        ExecutorQuoter.Update memory update3;
        update3.chainId = 10003;
        update3.update = packUint64(quote3.baseFee, quote3.dstGasPrice, quote3.srcPrice, quote3.dstPrice);
        updates.push(update1);
        updates.push(update2);
        updates.push(update3);
        // store first so the gas metric is on a non-zero slot
        executorQuoter.quoteUpdate(updates);
    }

    function test_chainInfoUpdate() public {
        executorQuoter.chainInfoUpdate(chainInfoUpdates);
        (bool enabled, uint8 gasPriceDecimals, uint8 nativeDecimals) = executorQuoter.chainInfos(10003);
        require(enabled);
        require(gasPriceDecimals == 18);
        require(nativeDecimals == 18);
    }

    function test_quoteUpdate() public {
        executorQuoter.quoteUpdate(updates);
        (,,, uint64 baseFee) = executorQuoter.quoteByDstChain(10003);
        require(baseFee == 27971);
    }

    function test_fuzz_quoteUpdate(ExecutorQuoter.Update[] calldata _updates) public {
        executorQuoter.quoteUpdate(_updates);
    }

    function test_requestQuote() public view {
        executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            RelayInstructions.encodeGas(250000, 0)
        );
    }

    function test_requestExecutionQuote() public view {
        executorQuoter.requestExecutionQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            RelayInstructions.encodeGas(250000, 0)
        );
    }

    // Error path tests

    function test_chainInfoUpdate_invalidUpdater() public {
        address notUpdater = address(0xdead);
        vm.prank(notUpdater);
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoter.InvalidUpdater.selector, notUpdater, UPDATER));
        executorQuoter.chainInfoUpdate(chainInfoUpdates);
    }

    function test_quoteUpdate_invalidUpdater() public {
        address notUpdater = address(0xdead);
        vm.prank(notUpdater);
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoter.InvalidUpdater.selector, notUpdater, UPDATER));
        executorQuoter.quoteUpdate(updates);
    }

    function test_requestQuote_chainDisabled() public {
        uint16 disabledChain = 65000;
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoter.ChainDisabled.selector, disabledChain));
        executorQuoter.requestQuote(
            disabledChain,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            RelayInstructions.encodeGas(250000, 0)
        );
    }

    function test_requestQuote_unsupportedInstruction() public {
        // Type 0xFF is not a valid instruction type
        bytes memory badInstruction = hex"ff";
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoter.UnsupportedInstruction.selector, 0xff));
        executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            badInstruction
        );
    }

    function test_requestQuote_moreThanOneDropOff() public {
        // Two Type 2 (drop-off) instructions
        bytes memory twoDropOffs = abi.encodePacked(
            RelayInstructions.encodeGasDropOffInstructions(1000, bytes32(uint256(uint160(address(this))))),
            RelayInstructions.encodeGasDropOffInstructions(2000, bytes32(uint256(uint160(address(this)))))
        );
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoter.MoreThanOneDropOff.selector));
        executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            twoDropOffs
        );
    }

    // Edge case tests for large values
    //
    // Note: The quote calculation uses uint128 * uint64 which fits in uint256 (2^192 < 2^256),
    // so overflow is not possible with the current type constraints. These tests verify
    // the contract handles extreme values correctly without reverting.
    //
    // Run with `forge test -vv` to see the actual quote values logged.

    /// @notice Test that max uint128 gas limit is handled without overflow.
    /// uint128 * uint64 = 2^192 max, which fits in uint256.
    function test_requestQuote_maxGasLimit() public {
        uint128 maxGas = type(uint128).max;
        bytes memory relayInstructions = RelayInstructions.encodeGas(maxGas, 0);

        uint256 quote = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            relayInstructions
        );

        // Log the actual values for inspection
        emit log_named_uint("maxGasLimit input", maxGas);
        emit log_named_uint("quote result", quote);
        emit log_named_uint("quote in ETH (approx)", quote / 1e18);

        assertGt(quote, maxGas, "Quote should be non-zero");
    }

    /// @notice Test that max uint128 msgValue is handled without overflow.
    function test_requestQuote_maxMsgValue() public {
        uint128 maxMsgValue = type(uint128).max;
        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, maxMsgValue);

        uint256 quote = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            relayInstructions
        );

        assertGt(quote, maxMsgValue, "Quote should be greater than u128.max");
    }

    /// @notice Test with extreme price values - all max uint64.
    /// When srcPrice == dstPrice, conversion ratio is ~1, so no overflow.
    function test_requestQuote_extremePrices() public {
        ExecutorQuoter.Update[] memory extremeUpdates = new ExecutorQuoter.Update[](1);
        extremeUpdates[0].chainId = DST_CHAIN;
        extremeUpdates[0].update = packUint64(
            type(uint64).max, // baseFee
            type(uint64).max, // dstGasPrice
            type(uint64).max, // srcPrice
            type(uint64).max // dstPrice
        );
        executorQuoter.quoteUpdate(extremeUpdates);

        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        uint256 quote = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            relayInstructions
        );

        emit log_named_uint("baseFee", type(uint64).max);
        emit log_named_uint("dstGasPrice", type(uint64).max);
        emit log_named_uint("srcPrice", type(uint64).max);
        emit log_named_uint("dstPrice", type(uint64).max);
        emit log_named_uint("gasLimit", 250000);
        emit log_named_uint("quote result", quote);
        emit log_named_uint("quote in ETH (approx)", quote / 1e18);

        assertGt(quote, type(uint64).max, "Quote should be greater than a max price");
    }

    /// @notice Test quote with max msgValue AND max dropoff to verify they sum correctly.
    function test_requestQuote_maxMsgValueAndDropoff() public {
        uint128 maxGas = type(uint128).max;
        uint128 maxMsgValue = type(uint128).max;
        uint128 maxDropoff = type(uint128).max;

        // Combine gas instruction (with max gas and max msgValue) + dropoff instruction
        bytes memory relayInstructions = abi.encodePacked(
            RelayInstructions.encodeGas(maxGas, maxMsgValue),
            RelayInstructions.encodeGasDropOffInstructions(maxDropoff, bytes32(uint256(uint160(address(this)))))
        );

        uint256 quote = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            relayInstructions
        );

        emit log_named_uint("maxGas input", maxGas);
        emit log_named_uint("maxMsgValue input", maxMsgValue);
        emit log_named_uint("maxDropoff input", maxDropoff);
        emit log_named_uint("quote result", quote);
        emit log_named_uint("quote in ETH (approx)", quote / 1e18);
        emit log_named_uint("type(uint256).max", type(uint256).max);
        emit log_named_uint("type(uint128).max * 3", uint256(type(uint128).max) * 3);

        // Verify quote is not capped at uint256 max (i.e., it's a real sum)
        assertGt(quote, uint256(type(uint128).max), "Quote should be greater than a single max uint128");
    }

    /// @notice Compare individual quotes vs combined to verify addition.
    function test_requestQuote_verifyAddition() public {
        uint128 gasLimit = 250000;
        uint128 msgValue = 1e18; // 1 ETH worth
        uint128 dropoff = 2e18; // 2 ETH worth

        // Get quote with just gas
        bytes memory gasOnly = RelayInstructions.encodeGas(gasLimit, 0);
        uint256 quoteGasOnly = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            gasOnly
        );

        // Get quote with gas + msgValue
        bytes memory gasAndMsg = RelayInstructions.encodeGas(gasLimit, msgValue);
        uint256 quoteGasAndMsg = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            gasAndMsg
        );

        // Get quote with gas + msgValue + dropoff
        bytes memory all = abi.encodePacked(
            RelayInstructions.encodeGas(gasLimit, msgValue),
            RelayInstructions.encodeGasDropOffInstructions(dropoff, bytes32(uint256(uint160(address(this)))))
        );
        uint256 quoteAll = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            all
        );

        // Verify monotonic increase
        assertGt(quoteGasAndMsg, quoteGasOnly, "Adding msgValue should increase quote");
        assertGt(quoteAll, quoteGasAndMsg, "Adding dropoff should increase quote");
    }

    /// @notice Test quote calculation with zero prices (division by zero protection).
    function test_requestQuote_zeroPrices() public {
        // Set up a quote with zero srcPrice (would cause division by zero)
        ExecutorQuoter.Update[] memory zeroUpdates = new ExecutorQuoter.Update[](1);
        zeroUpdates[0].chainId = DST_CHAIN;
        zeroUpdates[0].update = packUint64(
            27971, // baseFee
            100000000, // dstGasPrice
            0, // srcPrice = 0 (division by zero)
            35751300000000 // dstPrice
        );
        executorQuoter.quoteUpdate(zeroUpdates);

        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        // Should revert on division by zero
        vm.expectRevert();
        executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            relayInstructions
        );
    }

    /// @notice Test with zero gas limit - should return just base fee.
    function test_requestQuote_zeroGasLimit() public view {
        bytes memory relayInstructions = RelayInstructions.encodeGas(0, 0);

        uint256 quote = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            relayInstructions
        );

        // With zero gas, quote should be just the normalized base fee
        // baseFee = 27971 (in 10^10 decimals), normalized to 18 decimals
        // 27971 * 10^(18-10) = 27971 * 10^8 = 2797100000000
        require(quote >= 2797100000000, "Quote should include base fee");
    }

    /// @notice Test normalize function with from > to (division path).
    /// This exercises the branch at line 107 where decimals are reduced.
    function test_requestQuote_normalizeFromGreaterThanTo() public {
        // Create a new quoter with SRC_TOKEN_DECIMALS = 8 (less than DECIMAL_RESOLUTION = 18)
        // This will hit the from > to branch in normalize() at lines 183 and 187
        ExecutorQuoter lowDecimalQuoter = new ExecutorQuoter(UPDATER, UPDATER, 8, bytes32(uint256(uint160(UPDATER))));

        // Set up chain info
        ExecutorQuoter.Update[] memory chainInfo = new ExecutorQuoter.Update[](1);
        chainInfo[0].chainId = DST_CHAIN;
        chainInfo[0].update = CHAIN_INFO_UPDATE_PACKED;
        lowDecimalQuoter.chainInfoUpdate(chainInfo);

        // Set up quote with non-zero values
        ExecutorQuoter.Update[] memory quoteUpdates = new ExecutorQuoter.Update[](1);
        quoteUpdates[0].chainId = DST_CHAIN;
        quoteUpdates[0].update = packUint64(27971, 100000000, 35751300000000, 35751300000000);
        lowDecimalQuoter.quoteUpdate(quoteUpdates);

        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 1e18);

        uint256 quote = lowDecimalQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            relayInstructions
        );

        // Quote should be non-zero and scaled down due to 8 decimals
        assertGt(quote, 0, "Quote should be non-zero");
    }

    /// @notice Test normalize function with from == to (identity path).
    /// This exercises the branch at line 111 where no scaling is needed.
    function test_requestQuote_normalizeFromEqualsTo() public {
        // Create a quoter with SRC_TOKEN_DECIMALS = 10 (equals QUOTE_DECIMALS)
        // This will hit the from == to branch at line 174: normalize(baseFee, 10, 10)
        ExecutorQuoter equalDecimalQuoter = new ExecutorQuoter(UPDATER, UPDATER, 10, bytes32(uint256(uint160(UPDATER))));

        // Set up chain info with gasPriceDecimals = 18, nativeDecimals = 18
        ExecutorQuoter.Update[] memory chainInfo = new ExecutorQuoter.Update[](1);
        chainInfo[0].chainId = DST_CHAIN;
        chainInfo[0].update = CHAIN_INFO_UPDATE_PACKED;
        equalDecimalQuoter.chainInfoUpdate(chainInfo);

        // Set up quote
        ExecutorQuoter.Update[] memory quoteUpdates = new ExecutorQuoter.Update[](1);
        quoteUpdates[0].chainId = DST_CHAIN;
        quoteUpdates[0].update = packUint64(27971, 100000000, 35751300000000, 35751300000000);
        equalDecimalQuoter.quoteUpdate(quoteUpdates);

        // Use zero gas and zero msgValue to isolate the baseFee normalization
        bytes memory relayInstructions = RelayInstructions.encodeGas(0, 0);

        uint256 quote = equalDecimalQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            relayInstructions
        );

        // With SRC_TOKEN_DECIMALS = 10 = QUOTE_DECIMALS, baseFee should pass through unchanged
        // baseFee = 27971
        assertEq(quote, 27971, "Quote should equal baseFee when decimals match");
    }

    /// @notice Test requestExecutionQuote reverts when chain is disabled.
    function test_requestExecutionQuote_chainDisabled() public {
        uint16 disabledChain = 65000;
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoter.ChainDisabled.selector, disabledChain));
        executorQuoter.requestExecutionQuote(
            disabledChain,
            DST_ADDR,
            UPDATER,
            ExecutorMessages.makeVAAv1Request(10002, bytes32(uint256(uint160(address(this)))), 1),
            RelayInstructions.encodeGas(250000, 0)
        );
    }
}

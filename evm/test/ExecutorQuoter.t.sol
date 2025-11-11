// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import {ExecutorQuoter} from "../src/ExecutorQuoter.sol";

contract ExecutorQuoterTest is Test {
    ExecutorQuoter public executorQuoter;
    ExecutorQuoter.Update[] public updates;
    ExecutorQuoter.Update[] public chainInfoUpdates;

    address constant updater = 0x7FA9385bE102ac3EAc297483Dd6233D62b3e1496;
    bytes32 constant chainInfoUpdatePacked = 0x0000000000000000000000000000000000000000000000000000000000121201;
    uint16 constant dstChain = 10003;
    bytes32 constant dstAddr = bytes32(0);

    function packUint64(uint64 a, uint64 b, uint64 c, uint64 d) public pure returns (bytes32) {
        return bytes32((uint256(d) << 192) | (uint256(c) << 128) | (uint256(b) << 64) | uint256(a));
    }

    function setUp() public {
        executorQuoter = new ExecutorQuoter(updater, updater, 18, bytes32(uint256(uint160(updater))));
        ExecutorQuoter.Update memory chainInfoUpdate;
        chainInfoUpdate.chainId = 10003;
        chainInfoUpdate.update = chainInfoUpdatePacked;
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
        (uint64 baseFee,,,) = executorQuoter.quoteByDstChain(10003);
        require(baseFee == 27971);
    }

    function test_fuzz_quoteUpdate(ExecutorQuoter.Update[] calldata _updates) public {
        executorQuoter.quoteUpdate(_updates);
    }

    function test_requestQuote() public view {
        executorQuoter.requestQuote(
            dstChain, dstAddr, updater, abi.encodePacked(""), abi.encodePacked(uint8(1), uint128(250000))
        );
    }
}

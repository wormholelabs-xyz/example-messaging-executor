// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {IExecutor} from "../src/interfaces/IExecutor.sol";
import {Executor} from "../src/Executor.sol";

contract ExecutorTest is Test {
    Executor public executor;

    function setUp() public {
        executor = new Executor(2);
    }

    function test_requestExecution() public {
        Executor.SignedQuote memory signedQuote = IExecutor.SignedQuote({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 2,
            dstChain: 4,
            baseFee: 0,
            conversionRate: 0,
            expiryTime: uint64(block.timestamp + 1),
            signature: IExecutor.Signature({r: 0, s: 0, v: 0})
        });
        executor.requestExecution(4, bytes32(0), 0, 0, address(0), signedQuote, hex"");
    }

    function test_requestExecutionRevertsWithSrcMismatch() public {
        Executor.SignedQuote memory signedQuote = IExecutor.SignedQuote({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 4,
            dstChain: 2,
            baseFee: 0,
            conversionRate: 0,
            expiryTime: uint64(block.timestamp + 1),
            signature: IExecutor.Signature({r: 0, s: 0, v: 0})
        });
        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteSrcChainMismatch.selector, 4, 2));
        executor.requestExecution(4, bytes32(0), 0, 0, address(0), signedQuote, hex"");
    }

    function test_requestExecutionRevertsWithDstMismatch() public {
        Executor.SignedQuote memory signedQuote = IExecutor.SignedQuote({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 2,
            dstChain: 4,
            baseFee: 0,
            conversionRate: 0,
            expiryTime: uint64(block.timestamp + 1),
            signature: IExecutor.Signature({r: 0, s: 0, v: 0})
        });
        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteDstChainMismatch.selector, 4, 5));
        executor.requestExecution(5, bytes32(0), 0, 0, address(0), signedQuote, hex"");
    }

    function test_requestExecutionRevertsWithExpiredQuote() public {
        Executor.SignedQuote memory signedQuote = IExecutor.SignedQuote({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 2,
            dstChain: 4,
            baseFee: 0,
            conversionRate: 0,
            expiryTime: uint64(block.timestamp),
            signature: IExecutor.Signature({r: 0, s: 0, v: 0})
        });
        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteExpired.selector, uint64(block.timestamp)));
        executor.requestExecution(4, bytes32(0), 0, 0, address(0), signedQuote, hex"");
    }

    function test_requestExecutionRevertsWithNonEvmPayee() public {
        Executor.SignedQuote memory signedQuote = IExecutor.SignedQuote({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
            srcChain: 2,
            dstChain: 4,
            baseFee: 0,
            conversionRate: 0,
            expiryTime: uint64(block.timestamp + 1),
            signature: IExecutor.Signature({r: 0, s: 0, v: 0})
        });
        vm.expectRevert(
            abi.encodeWithSelector(
                Executor.NotAnEvmAddress.selector,
                bytes32(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff)
            )
        );
        executor.requestExecution(4, bytes32(0), 0, 0, address(0), signedQuote, hex"");
    }
}

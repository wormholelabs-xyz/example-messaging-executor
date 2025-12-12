// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import {IExecutor} from "../src/interfaces/IExecutor.sol";
import {Executor} from "../src/Executor.sol";

contract ExecutorTest is Test {
    Executor public executor;

    function setUp() public {
        executor = new Executor(2);
    }

    function encodeSignedQuoteHeader(Executor.SignedQuoteHeader memory signedQuote)
        internal
        pure
        returns (bytes memory)
    {
        return abi.encodePacked(
            signedQuote.prefix,
            signedQuote.quoterAddress,
            signedQuote.payeeAddress,
            signedQuote.srcChain,
            signedQuote.dstChain,
            signedQuote.expiryTime
        );
    }

    function test_requestExecution() public {
        Executor.SignedQuoteHeader memory signedQuote = IExecutor.SignedQuoteHeader({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 2,
            dstChain: 4,
            expiryTime: uint64(block.timestamp + 1)
        });
        executor.requestExecution(4, bytes32(0), address(0), encodeSignedQuoteHeader(signedQuote), hex"", hex"");
    }

    function test_requestExecutionWithRelayInstructions() public {
        Executor.SignedQuoteHeader memory signedQuote = IExecutor.SignedQuoteHeader({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 2,
            dstChain: 4,
            expiryTime: uint64(block.timestamp + 1)
        });
        executor.requestExecution(
            4, bytes32(0), address(0), encodeSignedQuoteHeader(signedQuote), hex"", "Hello, World!"
        );
    }

    function test_requestExecutionRevertsWithSrcMismatch() public {
        Executor.SignedQuoteHeader memory signedQuote = IExecutor.SignedQuoteHeader({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 4,
            dstChain: 2,
            expiryTime: uint64(block.timestamp + 1)
        });
        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteSrcChainMismatch.selector, 4, 2));
        executor.requestExecution(4, bytes32(0), address(0), encodeSignedQuoteHeader(signedQuote), hex"", hex"");
    }

    function test_requestExecutionRevertsWithDstMismatch() public {
        Executor.SignedQuoteHeader memory signedQuote = IExecutor.SignedQuoteHeader({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 2,
            dstChain: 4,
            expiryTime: uint64(block.timestamp + 1)
        });
        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteDstChainMismatch.selector, 4, 5));
        executor.requestExecution(5, bytes32(0), address(0), encodeSignedQuoteHeader(signedQuote), hex"", hex"");
    }

    function test_requestExecutionRevertsWithExpiredQuote() public {
        Executor.SignedQuoteHeader memory signedQuote = IExecutor.SignedQuoteHeader({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0),
            srcChain: 2,
            dstChain: 4,
            expiryTime: uint64(block.timestamp)
        });
        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteExpired.selector, uint64(block.timestamp)));
        executor.requestExecution(4, bytes32(0), address(0), encodeSignedQuoteHeader(signedQuote), hex"", hex"");
    }

    function test_requestExecutionRevertsWithNonEvmPayee() public {
        Executor.SignedQuoteHeader memory signedQuote = IExecutor.SignedQuoteHeader({
            prefix: "EQ01",
            quoterAddress: address(0),
            payeeAddress: bytes32(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff),
            srcChain: 2,
            dstChain: 4,
            expiryTime: uint64(block.timestamp + 1)
        });
        vm.expectRevert(
            abi.encodeWithSelector(
                Executor.NotAnEvmAddress.selector,
                bytes32(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff)
            )
        );
        executor.requestExecution(4, bytes32(0), address(0), encodeSignedQuoteHeader(signedQuote), hex"", hex"");
    }
}

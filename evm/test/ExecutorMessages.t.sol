// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test, console} from "forge-std/Test.sol";
import {ExecutorMessages} from "../src/libraries/ExecutorMessages.sol";

contract ExecutorMessagesTest is Test {
    function test_makeMMRequest() public pure {
        address srcAddr = address(0xdeadbeef);
        uint16 srcChain = 2;
        uint64 sequence = 42;
        bytes memory payload = "Hello, World";
        bytes memory expected = abi.encodePacked(
            "ERM1", // prefix
            srcChain, // sourceChainId
            bytes32(uint256(uint160(srcAddr))), // sourceAddress
            sequence, // sequence
            uint32(payload.length), // payloadLen
            payload // payload
        );
        bytes memory buf = ExecutorMessages.makeMMRequest(srcChain, srcAddr, sequence, payload);
        assertEq(keccak256(expected), keccak256(buf));
    }

    function test_makeVAAv1Request() public pure {
        uint16 emitterChain = 7;
        bytes32 emitterAddress = bytes32(uint256(uint160(0xdeadbeef)));
        bytes memory expected = abi.encodePacked("ERV1", emitterChain, emitterAddress, uint64(42));
        bytes memory buf = ExecutorMessages.makeVAAv1Request(emitterChain, emitterAddress, 42);
        assertEq(keccak256(expected), keccak256(buf));
    }

    function test_makeNTTv1Request() public pure {
        uint16 srcChain = 7;
        bytes32 srcManager = bytes32(uint256(uint160(0xdeadbeef)));
        bytes32 messageId = bytes32(uint256(42));
        bytes memory expected = abi.encodePacked("ERN1", srcChain, srcManager, messageId);
        bytes memory buf = ExecutorMessages.makeNTTv1Request(srcChain, srcManager, messageId);
        assertEq(keccak256(expected), keccak256(buf));
    }

    function test_makeCCTPv1Request() public pure {
        uint32 srcDomain = 7;
        uint64 nonce = 42;
        bytes memory expected = abi.encodePacked("ERC1", srcDomain, nonce);
        bytes memory buf = ExecutorMessages.makeCCTPv1Request(srcDomain, nonce);
        assertEq(keccak256(expected), keccak256(buf));
    }
}

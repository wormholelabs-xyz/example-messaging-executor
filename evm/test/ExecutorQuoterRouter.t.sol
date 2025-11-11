// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import {Executor} from "../src/Executor.sol";
import {ExecutorQuoterRouter} from "../src/ExecutorQuoterRouter.sol";

contract ExecutorQuoterRouterTest is Test {
    Executor public executor;
    ExecutorQuoterRouter public executorQuoterRouter;

    uint16 constant OUR_CHAIN = 10002;
    bytes32 constant UPDATE_IMPLEMENTATION = 0x000000000000000000000000aaa039ee238299b23cb4f9cd40775589efa962fd;
    bytes32 constant BAD_UPDATE_IMPLEMENTATION = 0x100000000000000000000000aaa039ee238299b23cb4f9cd40775589efa962fd;
    uint64 constant EXPIRY = 1762880900;

    function setUp() public {
        executor = new Executor(OUR_CHAIN);
        executorQuoterRouter = new ExecutorQuoterRouter(address(executor));
    }

    function makeAndSignGovernance(uint16 chainId, address quoterAddr, bytes32 updateImplementation, uint256 quoterPk)
        private
        pure
        returns (bytes memory)
    {
        bytes memory govBody = abi.encodePacked(hex"45473031", chainId, quoterAddr, updateImplementation, EXPIRY);
        bytes32 digest = keccak256(govBody);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(quoterPk, digest);
        return abi.encodePacked(govBody, r, s, v);
    }

    function test_updateQuoterContract() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, UPDATE_IMPLEMENTATION, alicePk)
        );
    }

    function test_updateQuoterContractChainIdMismatch() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        uint16 badChain = OUR_CHAIN + 1;
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.ChainIdMismatch.selector, badChain, OUR_CHAIN));
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(badChain, alice, UPDATE_IMPLEMENTATION, alicePk)
        );
    }

    function test_updateQuoterContractBadImplementation() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        vm.expectRevert(
            abi.encodeWithSelector(ExecutorQuoterRouter.NotAnEvmAddress.selector, BAD_UPDATE_IMPLEMENTATION)
        );
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, BAD_UPDATE_IMPLEMENTATION, alicePk)
        );
    }

    function test_updateQuoterContractExpired() public {
        vm.warp(1762880901);
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.GovernanceExpired.selector, EXPIRY));
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, UPDATE_IMPLEMENTATION, alicePk)
        );
    }

    function test_updateQuoterContractBadSignature() public {
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.InvalidSignature.selector));
        executorQuoterRouter.updateQuoterContract(
            hex"4547303127125241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000aaa039ee238299b23cb4f9cd40775589efa962fd00000000691248922111b9ac29b0d785d41e8f8c66980f4651c9a35c066e875cab67fd625e5e59c62fc65912c14a2c2ee99acdd809397f932bcf35ba7d269f02f96e8688588145701b"
        );
    }

    function test_updateQuoterContractQuoterMismatch() public {
        (address alice,) = makeAddrAndKey("alice");
        (, uint256 bobPk) = makeAddrAndKey("bob");
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.InvalidSignature.selector));
        executorQuoterRouter.updateQuoterContract(makeAndSignGovernance(OUR_CHAIN, alice, UPDATE_IMPLEMENTATION, bobPk));
    }
}

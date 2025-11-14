// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import {ExecutorMessages} from "../src/libraries/ExecutorMessages.sol";
import {RelayInstructions} from "../src/libraries/RelayInstructions.sol";
import {Executor} from "../src/Executor.sol";
import {ExecutorQuoter} from "../src/ExecutorQuoter.sol";
import {ExecutorQuoterRouter} from "../src/ExecutorQuoterRouter.sol";

contract ExecutorQuoterRouterTest is Test {
    Executor public executor;
    ExecutorQuoter public executorQuoter;
    ExecutorQuoterRouter public executorQuoterRouter;
    address public testQuoter;
    uint256 public testQuoterPk;
    ExecutorQuoter.Update[] public updates;
    ExecutorQuoter.Update[] public chainInfoUpdates;

    uint16 constant OUR_CHAIN = 10002;
    bytes32 constant UPDATE_IMPLEMENTATION = 0x000000000000000000000000aaa039ee238299b23cb4f9cd40775589efa962fd;
    bytes32 constant BAD_UPDATE_IMPLEMENTATION = 0x100000000000000000000000aaa039ee238299b23cb4f9cd40775589efa962fd;
    bytes32 constant SENDER_ADDRESS = 0x0000000000000000000000007FA9385bE102ac3EAc297483Dd6233D62b3e1496;
    bytes32 constant BAD_SENDER_ADDRESS = 0x0000000000000000000000007FA9385bE102ac3EAc297483Dd6233D62b3e1490;
    uint64 constant EXPIRY = 1762880900;

    uint16 constant DST_CHAIN = 10003;
    bytes32 constant DST_ADDR = bytes32(0);

    function packUint64(uint64 a, uint64 b, uint64 c, uint64 d) public pure returns (bytes32) {
        return bytes32((uint256(d) << 192) | (uint256(c) << 128) | (uint256(b) << 64) | uint256(a));
    }

    function setUp() public {
        executor = new Executor(OUR_CHAIN);
        (testQuoter, testQuoterPk) = makeAddrAndKey("test");
        executorQuoter = new ExecutorQuoter(testQuoter, testQuoter, 18, bytes32(uint256(uint160(testQuoter))));
        executorQuoterRouter = new ExecutorQuoterRouter(address(executor));
        vm.startPrank(testQuoter);
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(
                OUR_CHAIN,
                testQuoter,
                bytes32(uint256(uint160(address(executorQuoter)))),
                bytes32(uint256(uint160(testQuoter))),
                testQuoterPk
            )
        );
        ExecutorQuoter.Update memory chainInfoUpdate;
        chainInfoUpdate.chainId = 10003;
        chainInfoUpdate.update = 0x0000000000000000000000000000000000000000000000000000000000121201;
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
        updates.push(update1);
        // store first so the gas metric is on a non-zero slot
        executorQuoter.quoteUpdate(updates);
        vm.stopPrank();
    }

    function makeAndSignGovernance(
        uint16 chainId,
        address quoterAddr,
        bytes32 updateImplementation,
        bytes32 senderAddress,
        uint256 quoterPk
    ) private pure returns (bytes memory) {
        bytes memory govBody =
            abi.encodePacked(hex"45473031", chainId, quoterAddr, updateImplementation, senderAddress, EXPIRY);
        bytes32 digest = keccak256(govBody);
        (uint8 v, bytes32 r, bytes32 s) = vm.sign(quoterPk, digest);
        return abi.encodePacked(govBody, r, s, v);
    }

    function test_updateQuoterContract() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, UPDATE_IMPLEMENTATION, SENDER_ADDRESS, alicePk)
        );
    }

    function test_updateQuoterContractChainIdMismatch() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        uint16 badChain = OUR_CHAIN + 1;
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.ChainIdMismatch.selector, badChain, OUR_CHAIN));
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(badChain, alice, UPDATE_IMPLEMENTATION, SENDER_ADDRESS, alicePk)
        );
    }

    function test_updateQuoterContractBadImplementation() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        vm.expectRevert(
            abi.encodeWithSelector(ExecutorQuoterRouter.NotAnEvmAddress.selector, BAD_UPDATE_IMPLEMENTATION)
        );
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, BAD_UPDATE_IMPLEMENTATION, SENDER_ADDRESS, alicePk)
        );
    }

    function test_updateQuoterContractInvalidSender() public {
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.InvalidSender.selector));
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, UPDATE_IMPLEMENTATION, BAD_SENDER_ADDRESS, alicePk)
        );
    }

    function test_updateQuoterContractExpired() public {
        vm.warp(1762880901);
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.GovernanceExpired.selector, EXPIRY));
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, UPDATE_IMPLEMENTATION, SENDER_ADDRESS, alicePk)
        );
    }

    function test_updateQuoterContractBadSignature() public {
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.InvalidSignature.selector));
        executorQuoterRouter.updateQuoterContract(
            hex"4547303127125241c9276698439fef2780dbab76fec90b633fbd000000000000000000000000aaa039ee238299b23cb4f9cd40775589efa962fd0000000000000000000000007FA9385bE102ac3EAc297483Dd6233D62b3e149600000000691248922111b9ac29b0d785d41e8f8c66980f4651c9a35c066e875cab67fd625e5e59c62fc65912c14a2c2ee99acdd809397f932bcf35ba7d269f02f96e8688588145701b"
        );
    }

    function test_updateQuoterContractQuoterMismatch() public {
        (address alice,) = makeAddrAndKey("alice");
        (, uint256 bobPk) = makeAddrAndKey("bob");
        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.InvalidSignature.selector));
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, UPDATE_IMPLEMENTATION, SENDER_ADDRESS, bobPk)
        );
    }

    function test_quoteExecution() public view {
        executorQuoterRouter.quoteExecution(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1),
            RelayInstructions.encodeGas(250000, 0)
        );
    }

    function test_requestExecution() public payable {
        executorQuoterRouter.requestExecution{value: 27797100000000}(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1),
            RelayInstructions.encodeGas(250000, 0)
        );
    }
}

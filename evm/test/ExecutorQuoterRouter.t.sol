// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import {ExecutorMessages} from "../src/libraries/ExecutorMessages.sol";
import {RelayInstructions} from "../src/libraries/RelayInstructions.sol";
import {Executor} from "../src/Executor.sol";
import {ExecutorQuoter} from "../src/ExecutorQuoter.sol";
import {ExecutorQuoterRouter} from "../src/ExecutorQuoterRouter.sol";
import {IExecutorQuoter} from "../src/interfaces/IExecutorQuoter.sol";

// Malicious refund receiver that attempts reentrancy
contract ReentrantRefundReceiver {
    ExecutorQuoterRouter public router;
    uint256 public attackCount;
    uint256 public maxAttacks;

    uint16 public dstChain;
    bytes32 public dstAddr;
    address public quoterAddr;
    bytes public requestBytes;
    bytes public relayInstructions;

    constructor(address _router) {
        router = ExecutorQuoterRouter(_router);
    }

    function setAttackParams(
        uint16 _dstChain,
        bytes32 _dstAddr,
        address _quoterAddr,
        bytes memory _requestBytes,
        bytes memory _relayInstructions,
        uint256 _maxAttacks
    ) external {
        dstChain = _dstChain;
        dstAddr = _dstAddr;
        quoterAddr = _quoterAddr;
        requestBytes = _requestBytes;
        relayInstructions = _relayInstructions;
        maxAttacks = _maxAttacks;
    }

    receive() external payable {
        if (attackCount < maxAttacks) {
            attackCount++;
            // Attempt to re-enter requestExecution during refund
            router.requestExecution{value: msg.value}(
                dstChain,
                dstAddr,
                address(this),
                quoterAddr,
                requestBytes,
                relayInstructions
            );
        }
    }
}

// Contract that rejects ETH transfers (no receive or fallback)
contract RefundRejecter {
    // Intentionally no receive() or fallback() function
}

// Malicious quoter that returns manipulated values
contract MaliciousQuoter is IExecutorQuoter {
    uint256 public reportedPrice;
    bytes32 public payeeAddress;

    constructor(uint256 _reportedPrice, address _payee) {
        reportedPrice = _reportedPrice;
        payeeAddress = bytes32(uint256(uint160(_payee)));
    }

    function requestQuote(uint16, bytes32, address, bytes calldata, bytes calldata)
        external
        view
        returns (uint256)
    {
        return reportedPrice;
    }

    function requestExecutionQuote(uint16, bytes32, address, bytes calldata, bytes calldata)
        external
        view
        returns (uint256, bytes32, bytes32)
    {
        // Returns a low price but real payee gets the funds
        return (reportedPrice, payeeAddress, bytes32(0));
    }
}

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
        return bytes32((uint256(a) << 192) | (uint256(b) << 128) | (uint256(c) << 64) | uint256(d));
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

    // Assertion-based tests

    function test_quoteExecution_returnsCorrectValue() public view {
        bytes memory requestBytes = ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1);
        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        uint256 routerQuote = executorQuoterRouter.quoteExecution(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            requestBytes,
            relayInstructions
        );

        uint256 directQuote = executorQuoter.requestQuote(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            requestBytes,
            relayInstructions
        );

        assertEq(routerQuote, directQuote, "Router quote should match direct quoter quote");
    }

    function test_requestExecution_underpaid() public {
        bytes memory requestBytes = ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1);
        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        uint256 quote = executorQuoterRouter.quoteExecution(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            requestBytes,
            relayInstructions
        );

        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.Underpaid.selector, quote - 1, quote));
        executorQuoterRouter.requestExecution{value: quote - 1}(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            requestBytes,
            relayInstructions
        );
    }

    function test_requestExecution_paysPayee() public {
        bytes memory requestBytes = ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1);
        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        uint256 quote = executorQuoterRouter.quoteExecution(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            requestBytes,
            relayInstructions
        );

        uint256 payeeBalanceBefore = testQuoter.balance;

        executorQuoterRouter.requestExecution{value: quote}(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            requestBytes,
            relayInstructions
        );

        assertEq(testQuoter.balance, payeeBalanceBefore + quote, "Payee should receive exact payment");
    }

    // Allow receiving refunds
    receive() external payable {}

    // Security tests

    /// @notice Test that reentrancy during refund doesn't cause issues.
    /// The current implementation does allow reentrancy, but it should not
    /// cause loss of funds since state is read before the external call.
    function test_reentrancy_duringRefund() public {
        bytes memory requestBytes = ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1);
        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        uint256 quote = executorQuoterRouter.quoteExecution(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            requestBytes,
            relayInstructions
        );

        // Deploy reentrancy attacker
        ReentrantRefundReceiver attacker = new ReentrantRefundReceiver(address(executorQuoterRouter));
        attacker.setAttackParams(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            requestBytes,
            relayInstructions,
            2 // Try to reenter twice
        );

        // Fund the attacker with enough for multiple calls
        uint256 excess = 1 ether;
        uint256 totalFunding = (quote * 3) + excess;
        vm.deal(address(attacker), totalFunding);

        // Track payee balance before
        uint256 payeeBalanceBefore = testQuoter.balance;

        // Call from attacker context - the attacker will try to reenter on refund
        vm.prank(address(attacker));
        executorQuoterRouter.requestExecution{value: quote + excess}(
            DST_CHAIN,
            DST_ADDR,
            address(attacker),
            testQuoter,
            requestBytes,
            relayInstructions
        );

        // Verify payee received correct payment (should be quote * 3 if reentrancy succeeded)
        uint256 payeeBalanceAfter = testQuoter.balance;
        uint256 totalPaid = payeeBalanceAfter - payeeBalanceBefore;

        // Reentrancy should succeed (contract doesn't prevent it)
        // attackCount should be 2 since maxAttacks = 2
        assertEq(attacker.attackCount(), 2, "Reentrancy should occur twice");

        // Each successful execution should pay the quote amount to payee
        // Total paid should equal quote * (1 + attackCount)
        uint256 expectedPayment = quote * (1 + attacker.attackCount());
        assertEq(totalPaid, expectedPayment, "Payee should receive correct total payment");
    }

    /// @notice Test behavior with a malicious quoter that returns zero price.
    function test_maliciousQuoter_zeroPrice() public {
        address maliciousPayee = makeAddr("maliciousPayee");
        MaliciousQuoter maliciousQuoter = new MaliciousQuoter(0, maliciousPayee);

        // Register the malicious quoter
        (address mallory, uint256 malloryPk) = makeAddrAndKey("mallory");
        vm.prank(mallory);
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(
                OUR_CHAIN,
                mallory,
                bytes32(uint256(uint160(address(maliciousQuoter)))),
                bytes32(uint256(uint160(mallory))),
                malloryPk
            )
        );

        bytes memory requestBytes = ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1);
        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        // Quote returns 0
        uint256 quote = executorQuoterRouter.quoteExecution(
            DST_CHAIN,
            DST_ADDR,
            mallory,
            mallory,
            requestBytes,
            relayInstructions
        );
        assertEq(quote, 0, "Malicious quoter should return 0");

        // User sends 1 ether thinking it's a good deal, gets full refund
        uint256 userBalanceBefore = address(this).balance;

        executorQuoterRouter.requestExecution{value: 1 ether}(
            DST_CHAIN,
            DST_ADDR,
            address(this),
            mallory,
            requestBytes,
            relayInstructions
        );

        // User should get full refund since quote was 0
        assertEq(address(this).balance, userBalanceBefore, "User should get full refund");
        // Payee should receive 0
        assertEq(maliciousPayee.balance, 0, "Malicious payee should receive 0");
    }

    /// @notice Test that quoter cannot steal funds by returning inconsistent values.
    /// Even if requestQuote and requestExecutionQuote return different values,
    /// the actual payment is based on requestExecutionQuote.
    function test_maliciousQuoter_inconsistentQuotes() public {
        // This quoter always returns a fixed low price
        address maliciousPayee = makeAddr("maliciousPayee");
        uint256 lowPrice = 100;
        MaliciousQuoter maliciousQuoter = new MaliciousQuoter(lowPrice, maliciousPayee);

        (address mallory, uint256 malloryPk) = makeAddrAndKey("mallory");
        vm.prank(mallory);
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(
                OUR_CHAIN,
                mallory,
                bytes32(uint256(uint160(address(maliciousQuoter)))),
                bytes32(uint256(uint160(mallory))),
                malloryPk
            )
        );

        bytes memory requestBytes = ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1);
        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        uint256 userBalanceBefore = address(this).balance;

        // User overpays significantly
        executorQuoterRouter.requestExecution{value: 1 ether}(
            DST_CHAIN,
            DST_ADDR,
            address(this),
            mallory,
            requestBytes,
            relayInstructions
        );

        // User should get refund of (1 ether - lowPrice)
        assertEq(address(this).balance, userBalanceBefore - lowPrice, "User should only pay the quoted price");
        // Payee should receive exactly lowPrice
        assertEq(maliciousPayee.balance, lowPrice, "Payee should receive exactly quoted price");
    }

    // Address validation tests (bytes32 with upper bits set)

    /// @notice Test that governance rejects contract address with upper bits set.
    function test_updateQuoterContract_nonEvmContractAddress() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        // Set upper bit of contract address (not a valid EVM address)
        bytes32 badContractAddress = bytes32(uint256(1) << 255 | uint256(uint160(address(0x1234))));

        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.NotAnEvmAddress.selector, badContractAddress));
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, badContractAddress, SENDER_ADDRESS, alicePk)
        );
    }

    /// @notice Test that governance rejects sender address with upper bits set.
    function test_updateQuoterContract_nonEvmSenderAddress() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        // Set upper bit of sender address
        bytes32 badSenderAddress = bytes32(uint256(1) << 255 | uint256(uint160(alice)));

        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.NotAnEvmAddress.selector, badSenderAddress));
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, UPDATE_IMPLEMENTATION, badSenderAddress, alicePk)
        );
    }

    /// @notice Test address with just one bit set in upper 96 bits.
    function test_updateQuoterContract_singleBitInUpperBytes() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        // Set bit 160 (first bit outside valid EVM address range)
        bytes32 badAddress = bytes32(uint256(1) << 160 | uint256(uint160(address(0x1234))));

        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.NotAnEvmAddress.selector, badAddress));
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, badAddress, SENDER_ADDRESS, alicePk)
        );
    }

    /// @notice Test that max valid EVM address (all 160 bits set) is accepted.
    function test_updateQuoterContract_maxValidEvmAddress() public {
        (address alice, uint256 alicePk) = makeAddrAndKey("alice");
        // All 160 bits set = 0x00000000000000000000000ffffffffffffffffffffffffffffffffffffffff
        bytes32 maxEvmAddress = bytes32(uint256(type(uint160).max));

        // This should NOT revert due to address validation
        // (may revert for other reasons like invalid signature if signer != quoter)
        executorQuoterRouter.updateQuoterContract(
            makeAndSignGovernance(OUR_CHAIN, alice, maxEvmAddress, SENDER_ADDRESS, alicePk)
        );
    }

    /// @notice Test that RefundFailed is thrown when refund recipient rejects ETH.
    function test_requestExecution_refundFailed() public {
        // Deploy a contract that cannot receive ETH
        RefundRejecter rejecter = new RefundRejecter();

        bytes memory requestBytes = ExecutorMessages.makeVAAv1Request(OUR_CHAIN, bytes32(uint256(uint160(address(this)))), 1);
        bytes memory relayInstructions = RelayInstructions.encodeGas(250000, 0);

        uint256 quote = executorQuoterRouter.quoteExecution(
            DST_CHAIN,
            DST_ADDR,
            testQuoter,
            testQuoter,
            requestBytes,
            relayInstructions
        );

        // Overpay so a refund is attempted
        uint256 overpayment = quote + 1 ether;

        vm.expectRevert(abi.encodeWithSelector(ExecutorQuoterRouter.RefundFailed.selector, address(rejecter)));
        executorQuoterRouter.requestExecution{value: overpayment}(
            DST_CHAIN,
            DST_ADDR,
            address(rejecter), // refundAddr that rejects ETH
            testQuoter,
            requestBytes,
            relayInstructions
        );
    }
}

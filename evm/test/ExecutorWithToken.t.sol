// SPDX-License-Identifier: Apache-2.0
pragma solidity ^0.8.13;

import {Test} from "forge-std/Test.sol";
import {Executor} from "../src/Executor.sol";
import {ExecutorWithToken} from "../src/ExecutorWithToken.sol";
import {IExecutorWithToken} from "../src/interfaces/IExecutorWithToken.sol";
import {SafeTransferLib} from "solady/utils/SafeTransferLib.sol";

contract MockERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }
}

// Token that returns no data on transferFrom (for safeTransferFrom testing)
contract NonStandardERC20 {
    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }

    function approve(address spender, uint256 amount) external {
        allowance[msg.sender][spender] = amount;
    }

    function transferFrom(address from, address to, uint256 amount) external {
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
    }
}

// Token that fails on transferFrom
contract RevertingERC20 {
    function transferFrom(address, address, uint256) external pure {
        revert("always fails");
    }
}

// Token that transferFrom returns false without reverting
contract FalseReturningERC20 {
    function transferFrom(address, address, uint256) external pure returns (bool) {
        return false;
    }
}

contract ExecutorWithTokenTest is Test {
    Executor public executor;
    ExecutorWithToken public executorWithToken;
    MockERC20 public token;

    uint16 constant OUR_CHAIN = 2;
    uint16 constant DST_CHAIN = 4;
    bytes32 constant DST_ADDR = bytes32(0);
    uint256 constant AMOUNT = 1000000;

    function makeEQ03Quote(
        address quoterAddress,
        bytes32 payeeAddress,
        uint16 srcChain,
        uint16 dstChain,
        uint64 expiryTime
    ) internal pure returns (bytes memory) {
        return abi.encodePacked(
            bytes4("EQ03"),
            quoterAddress,
            payeeAddress,
            srcChain,
            dstChain,
            expiryTime,
            uint64(0), // baseFee
            uint64(0), // destinationGasPrice
            uint64(0), // sourcePrice
            uint64(0), // destinationPrice
            bytes32(0) // srcToken
        );
    }

    function setUp() public {
        executor = new Executor(OUR_CHAIN);
        executorWithToken = new ExecutorWithToken(address(executor));
        token = new MockERC20();
        token.mint(address(this), 10 * AMOUNT);
        token.approve(address(executorWithToken), type(uint256).max);
    }

    function test_requestExecution() public {
        address payee = makeAddr("payee");
        executorWithToken.requestExecution(
            AMOUNT,
            address(token),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );
    }

    function test_requestExecution_transfersTokensToPayee() public {
        address payee = makeAddr("payee");
        uint256 payeeBalanceBefore = token.balanceOf(payee);

        executorWithToken.requestExecution(
            AMOUNT,
            address(token),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );

        assertEq(token.balanceOf(payee), payeeBalanceBefore + AMOUNT, "Payee should receive token payment");
    }

    function test_requestExecution_emitsExecutionPayment() public {
        address payee = makeAddr("payee");
        address quoter = makeAddr("quoter");

        vm.expectEmit(true, false, false, true);
        emit IExecutorWithToken.ExecutionPayment(quoter, AMOUNT, address(token));

        executorWithToken.requestExecution(
            AMOUNT,
            address(token),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(quoter, bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)),
            hex"",
            hex""
        );
    }

    function test_requestExecution_nonStandardToken() public {
        NonStandardERC20 nsToken = new NonStandardERC20();
        nsToken.mint(address(this), AMOUNT);
        nsToken.approve(address(executorWithToken), AMOUNT);

        address payee = makeAddr("payee");

        executorWithToken.requestExecution(
            AMOUNT,
            address(nsToken),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );

        assertEq(nsToken.balanceOf(payee), AMOUNT, "Non-standard token should transfer correctly");
    }

    function test_requestExecution_withMsgValue() public {
        address payee = makeAddr("payee");
        uint256 payeeEthBefore = payee.balance;

        executorWithToken.requestExecution{value: 1 ether}(
            AMOUNT,
            address(token),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );

        assertEq(token.balanceOf(payee), AMOUNT, "Payee should receive token payment");
        assertEq(payee.balance, payeeEthBefore + 1 ether, "Payee should also receive ETH via Executor");
    }

    function test_versionConstant() public view {
        assertEq(
            keccak256(bytes(executorWithToken.EXECUTOR_WITH_TOKEN_VERSION())),
            keccak256(bytes("Executor-With-Token-0.0.1"))
        );
    }

    function test_executorImmutable() public view {
        assertEq(address(executorWithToken.EXECUTOR()), address(executor));
    }

    function test_requestExecution_revertingToken() public {
        RevertingERC20 badToken = new RevertingERC20();
        address payee = makeAddr("payee");

        vm.expectRevert(SafeTransferLib.TransferFromFailed.selector);
        executorWithToken.requestExecution(
            AMOUNT,
            address(badToken),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );
    }

    function test_requestExecution_falseReturningToken() public {
        FalseReturningERC20 badToken = new FalseReturningERC20();
        address payee = makeAddr("payee");

        vm.expectRevert(SafeTransferLib.TransferFromFailed.selector);
        executorWithToken.requestExecution(
            AMOUNT,
            address(badToken),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );
    }

    function test_requestExecution_insufficientAllowance() public {
        MockERC20 freshToken = new MockERC20();
        freshToken.mint(address(this), AMOUNT);
        address payee = makeAddr("payee");

        vm.expectRevert();
        executorWithToken.requestExecution(
            AMOUNT,
            address(freshToken),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );
    }

    function test_requestExecution_insufficientBalance() public {
        MockERC20 freshToken = new MockERC20();
        freshToken.approve(address(executorWithToken), AMOUNT);
        address payee = makeAddr("payee");

        vm.expectRevert();
        executorWithToken.requestExecution(
            AMOUNT,
            address(freshToken),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );
    }

    // Quote validation tests (delegated to Executor contract)
    function test_requestExecution_srcChainMismatch() public {
        address payee = makeAddr("payee");
        uint16 wrongSrcChain = OUR_CHAIN + 1;

        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteSrcChainMismatch.selector, wrongSrcChain, OUR_CHAIN));
        executorWithToken.requestExecution(
            AMOUNT,
            address(token),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), wrongSrcChain, DST_CHAIN, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );
    }

    function test_requestExecution_dstChainMismatch() public {
        address payee = makeAddr("payee");
        uint16 wrongDstChain = DST_CHAIN + 1;

        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteDstChainMismatch.selector, wrongDstChain, DST_CHAIN));
        executorWithToken.requestExecution(
            AMOUNT,
            address(token),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(
                address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, wrongDstChain, uint64(block.timestamp + 1)
            ),
            hex"",
            hex""
        );
    }

    function test_requestExecution_expiredQuote() public {
        address payee = makeAddr("payee");

        vm.expectRevert(abi.encodeWithSelector(Executor.QuoteExpired.selector, uint64(block.timestamp)));
        executorWithToken.requestExecution(
            AMOUNT,
            address(token),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(address(0), bytes32(uint256(uint160(payee))), OUR_CHAIN, DST_CHAIN, uint64(block.timestamp)),
            hex"",
            hex""
        );
    }

    // Address validation tests (ExecutorWithToken's own)
    function test_requestExecution_nonEvmPayee() public {
        bytes32 badPayee = bytes32(0xffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff);

        vm.expectRevert(abi.encodeWithSelector(ExecutorWithToken.NotAnEvmAddress.selector, badPayee));
        executorWithToken.requestExecution(
            AMOUNT,
            address(token),
            DST_CHAIN,
            DST_ADDR,
            address(this),
            makeEQ03Quote(address(0), badPayee, OUR_CHAIN, DST_CHAIN, uint64(block.timestamp + 1)),
            hex"",
            hex""
        );
    }
}

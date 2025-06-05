# Executor on EVM

## Integration

> 🚧 EVM integration is under active development and subject to change!

Integrating with Executor on EVM involves two aspects, requesting execution via the Executor contract and being executed by an off-chain relayer service.

To get started, `forge install https://github.com/wormholelabs-xyz/example-messaging-executor.git`.

### Requesting Execution

See the [design](../README.md) for more details on:

- [Signed Quote](../README.md#off-chain-quote)
- [Request For Execution](../README.md#request-for-execution)
- [Relay Instructions](../README.md#relay-instructions)

For ease of integration and flexibility, it is encouraged to pass in `relay_instructions` from off-chain.

#### Example v1 VAA Request

<!-- cspell:disable -->

```solidity
import "example-messaging-executor/evm/src/interfaces/IExecutor.sol";
import "example-messaging-executor/evm/src/libraries/ExecutorMessages.sol";
...
uint256 wormholeFee = wormhole.messageFee();
require(msg.value >= wormholeFee, "insufficient value");
uint256 executionAmount = msg.value - wormholeFee;

sequence = wormhole.publishMessage{
    value : wormholeFee
}(nonce, payload, finality);

executor.requestExecution{value: executionAmount}(
    dstChain,
    dstExecutionAddress,
    refundAddr,
    signedQuoteBytes,
    ExecutorMessages.makeVAAV1Request(chainId, bytes32(uint256(uint160(address(this)))), sequence),
    relayInstructions
);
```

<!-- cspell:enable -->

#### Example v1 NTT Request

<!-- cspell:disable -->

```solidity
import "example-messaging-executor/evm/src/interfaces/IExecutor.sol";
import "example-messaging-executor/evm/src/libraries/ExecutorMessages.sol";
...
uint64 msgId = nttm.transfer{value: msg.value - executorArgs.value}(
    amount, recipientChain, recipientAddress, refundAddress, shouldQueue, encodedInstructions
);

executor.requestExecution{value: executorArgs.value}(
    recipientChain,
    nttm.getPeer(recipientChain).peerAddress,
    executorArgs.refundAddress,
    executorArgs.signedQuote,
    ExecutorMessages.makeNTTv1Request(
        chainId, bytes32(uint256(uint160(address(nttm)))), bytes32(uint256(msgId))
    ),
    executorArgs.instructions
);
```

<!-- cspell:enable -->

#### Example v1 CCTP Request

<!-- cspell:disable -->

```solidity
import "example-messaging-executor/evm/src/interfaces/IExecutor.sol";
import "example-messaging-executor/evm/src/libraries/ExecutorMessages.sol";
...
uint64 nonce = circleTokenMessenger.depositForBurn(amount, destinationDomain, mintRecipient, burnToken);

executor.requestExecution{value: executorArgs.value}(
    0,
    bytes32(0),
    executorArgs.refundAddress,
    executorArgs.signedQuote,
    ExecutorMessages.makeCCTPv1Request(sourceDomain, nonce),
    executorArgs.instructions
);
```

<!-- cspell:enable -->

#### Example v2 CCTP Request

The `depositForBurn` function in CCTP v2 doesn't return anything, so we don't have a unique identifier for a transfer.
The off-chain executor will detect all Circle V2 transfers in the transaction and relay them.

<!-- cspell:disable -->

```solidity
import "example-messaging-executor/evm/src/interfaces/IExecutor.sol";
import "example-messaging-executor/evm/src/libraries/ExecutorMessages.sol";
...
circleTokenMessenger.depositForBurn(amount, destinationDomain, mintRecipient, burnToken, destinationCaller, maxFee, minFinalityThreshold);

executor.requestExecution{value: executorArgs.value}(
    0,
    bytes32(0),
    executorArgs.refundAddress,
    executorArgs.signedQuote,
    ExecutorMessages.makeCCTPv2Request(),
    executorArgs.instructions
);
```

<!-- cspell:enable -->

### Execution Support

#### v1 VAA Execution

Your contract must implement the following function.

```solidity
function executeVAAv1(bytes calldata encodedTransferMessage) public payable
```

#### v1 NTT Execution

The NTT Transceiver contract implements the following function.

```solidity
function receiveMessage(bytes memory encodedMessage) external
```

#### v1 CCTP Execution

The Circle Message Transmitter contract implements the following function.

```solidity
function receiveMessage(bytes calldata message, bytes calldata attestation) external override whenNotPaused returns (bool success)
```

#### v2 CCTP Execution

The Circle Message Transmitter contract implements the following function.

```solidity
function receiveMessage(bytes calldata message, bytes calldata attestation) external override whenNotPaused returns (bool success)
```

## Executor Development

### Testing

```shell
$ forge test
```

### Building

```shell
$ forge build
```

### Deploying

See [./sh/deployExecutor.sh](./sh/deployExecutor.sh)

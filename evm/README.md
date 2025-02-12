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

### Execution Support

#### v1 VAA Execution

Your contract must implement the following function.

```solidity
function receiveMessage(bytes calldata encodedTransferMessage) public payable
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

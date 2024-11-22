## EVM Testing

```
anvil --fork-url https://ethereum-rpc.publicnode.com
EVM_CHAIN_ID=1 MNEMONIC=0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 ./sh/deployExecutor.sh
```

update the address in `evm.ts`

http://localhost:3000/v0/quote/2/2
http://localhost:3000/v0/request/VAAv1/30/000000000000000000000000706f82e9bb5b0813501714ab5974216704980e31/137279
http://localhost:3000/v0/request/MM/2/000000000000000000000000706f82e9bb5b0813501714ab5974216704980e31/1/48656C6C6F20576F726C6421
http://localhost:3000/v0/estimate/{quote}/250000/0

```
cast send --rpc-url http://localhost:8545 --private-key 0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80 --value {estimate} {executorAddress} "requestExecution(uint16,bytes32,uint256,uint256,address,bytes,bytes)" 2 0x000000000000000000000000F94AB55a20B32AC37c3A105f12dB535986697945 250000 0 0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266 {quote} 0x45525631001e000000000000000000000000706f82e9bb5b0813501714ab5974216704980e31000000000002183f
```

fetching status also kicks off the relay (which right now just logs the cast command to run)

http://localhost:3000/v0/status/0002{txhashWithout0x}

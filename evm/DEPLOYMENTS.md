# Executor EVM Deployments

## Mainnet

### May 8, 2025

#### Version Info

Commit Hash:

<!-- cspell:disable -->

```sh
evm (main)$ git rev-parse HEAD
5428c3bc37097023ef68eddc33b11fe0d69fdd58
evm (main)$
```

<!-- cspell:enable -->

Foundry Version:

<!-- cspell:disable -->

```sh
evm (main)$ forge --version
forge Version: 1.0.0-stable
Commit SHA: e144b82070619b6e10485c38734b4d4d45aebe04
Build Timestamp: 2025-02-13T20:02:34.979686000Z (1739476954)
Build Profile: maxperf
evm (main)$
```

<!-- cspell:enable -->

#### Chains Deployed

Here are the deployed contract addresses for each chain. The number after the chain name is the Wormhole chain ID configured for the contract.

- Ethereum (2): [0x84eee8dba37c36947397e1e11251ca9a06fc6f8a](https://etherscan.io/address/0x84eee8dba37c36947397e1e11251ca9a06fc6f8a)
- Polygon (5): [0x0B23efA164aB3eD08e9a39AC7aD930Ff4F5A5e81](https://polygonscan.com/address/0x0b23efa164ab3ed08e9a39ac7ad930ff4f5a5e81)
- Avalanche (6): [0x4661F0E629E4ba8D04Ee90080Aee079740B00381](https://snowtrace.io/address/0x4661F0E629E4ba8D04Ee90080Aee079740B00381)
- Arbitrum (23): [0x3980f8318fc03d79033Bbb421A622CDF8d2Eeab4](https://arbiscan.io/address/0x3980f8318fc03d79033bbb421a622cdf8d2eeab4)
- Optimism (24): [0x85B704501f6AE718205C0636260768C4e72ac3e7](https://optimistic.etherscan.io/address/0x85b704501f6ae718205c0636260768c4e72ac3e7)
- Base (30): [0x9E1936E91A4a5AE5A5F75fFc472D6cb8e93597ea](https://basescan.org/address/0x9e1936e91a4a5ae5a5f75ffc472d6cb8e93597ea)
- Linea (38): [0x23aF2B5296122544A9A7861da43405D5B15a9bD3](https://lineascan.build/address/0x23af2b5296122544a9a7861da43405d5b15a9bd3)
- Unichain (44): [0x764dD868eAdD27ce57BCB801E4ca4a193d231Aed](https://uniscan.xyz/address/0x764dd868eadd27ce57bcb801e4ca4a193d231aed)
- World Chain (45): [0x8689b4E6226AdC8fa8FF80aCc3a60AcE31e8804B](https://worldscan.org/address/0x8689b4e6226adc8fa8ff80acc3a60ace31e8804b)
- Sonic (52): [0x3Fdc36b4260Da38fBDba1125cCBD33DD0AC74812](https://sonicscan.org/address/0x3fdc36b4260da38fbdba1125ccbd33dd0ac74812)

## Testnet

### September 11, 2025

#### Version Info

Commit Hash:

<!-- cspell:disable -->

```sh
evm (main)$ git rev-parse HEAD
575069616477efbec961bcfb77d7baf44e9f3baa
evm (main)$
```

<!-- cspell:enable -->

Foundry Version:

<!-- cspell:disable -->

```sh
evm (main)$ forge --version
forge Version: 1.3.5-stable
Commit SHA: 9979a41b5daa5da1572d973d7ac5a3dd2afc0221
Build Timestamp: 2025-09-09T04:49:44.505104000Z (1757393384)
Build Profile: maxperf
evm (main)$
```

<!-- cspell:enable -->

#### Chains Deployed

Here are the deployed contract addresses for each chain. The number after the chain name is the Wormhole chain ID configured for the contract.

- Sepolia (10002): [0xD0fb39f5a3361F21457653cB70F9D0C9bD86B66B](https://sepolia.etherscan.io/address/0xD0fb39f5a3361F21457653cB70F9D0C9bD86B66B)
- Base Sepolia (10004): [0x51B47D493CBA7aB97e3F8F163D6Ce07592CE4482](https://sepolia.basescan.org/address/0x51B47D493CBA7aB97e3F8F163D6Ce07592CE4482)
- Avalanche Fuji (6): [0x4661F0E629E4ba8D04Ee90080Aee079740B00381](https://testnet.snowtrace.io/address/0x4661F0E629E4ba8D04Ee90080Aee079740B00381)
- Mezo Testnet (50):
  [0x0f9b8E144Cc5C5e7C0073829Afd30F26A50c5606](https://api.explorer.test.mezo.org/address/0x0f9b8e144cc5c5e7c0073829afd30f26a50c5606)

### Bytecode Verification

If you wish to verify that the bytecode built locally matches what is deployed on chain, you can do something like this:

<!-- cspell:disable -->

```
forge verify-bytecode <contract_addr> Executor --rpc-url <archive_node_rpc> --verifier-api-key <your_etherscan_key>
```

<!-- cspell:enable -->

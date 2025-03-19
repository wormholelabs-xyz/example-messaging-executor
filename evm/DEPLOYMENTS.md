# Executor EVM Deployments

## Testnet

### March 18, 2025

#### Version Info

Commit Hash:

<!-- cspell:disable -->

```sh
evm (main)$ git rev-parse HEAD
a85f445dca1e9a3a6ed1d4912dc4826c0ce5e10f
evm (main)$
```

<!-- cspell:enable -->

Foundry Version:

<!-- cspell:disable -->

```sh
evm (main)$ forge --version
forge Version: 1.0.0-stable
Commit SHA: e144b82070619b6e10485c38734b4d4d45aebe04
Build Timestamp: 2025-02-13T20:03:31.026474817Z (1739477011)
Build Profile: maxperf
evm (main)$
```

<!-- cspell:enable -->

#### Chains Deployed

Here are the deployed contract addresses for each chain. The number after the chain name is the Wormhole chain ID configured for the contract.

- Sepolia (10002): [0xD0fb39f5a3361F21457653cB70F9D0C9bD86B66B](https://sepolia.etherscan.io/address/0xD0fb39f5a3361F21457653cB70F9D0C9bD86B66B)
- Base Sepolia (10004): [0x51B47D493CBA7aB97e3F8F163D6Ce07592CE4482](https://sepolia.basescan.org/address/0x51B47D493CBA7aB97e3F8F163D6Ce07592CE4482)
- Avalanche Fuji (6): [0x4661F0E629E4ba8D04Ee90080Aee079740B00381](https://testnet.snowtrace.io/address/0x4661F0E629E4ba8D04Ee90080Aee079740B00381)

### Bytecode Verification

If you wish to verify that the bytecode built locally matches what is deployed on chain, you can do something like this:

<!-- cspell:disable -->

```
forge verify-bytecode <contract_addr> Executor --rpc-url <archive_node_rpc> --verifier-api-key <your_etherscan_key>
```

<!-- cspell:enable -->

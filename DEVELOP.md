# Executor Development Notes

## Overview

This repository is generated from this [multi-chain template](https://github.com/evan-gray/multichain-template).

## Runtime Support

- [x] [EVM](https://ethereum.org/en/developers/docs/evm/)
- [x] [SVM](https://solana.com/developers/evm-to-svm/smart-contracts)
- [ ] [Sui Move](https://sui.io/move)
- [ ] [Aptos Move](https://aptos.dev/en/build/smart-contracts)

## Developer Dependencies

### Off-Chain

- [Bun](https://bun.sh/docs/installation)

Run `bun install --frozen-lockfile` at the root of this repo to install the off-chain dependencies.

### EVM

- [Foundry](https://book.getfoundry.sh/getting-started/installation)

### SVM

- [Rust 1.75.0](https://www.rust-lang.org/tools/install)
- [Solana 1.18.17](https://solana.com/docs/intro/installation)
- [Yarn](https://yarnpkg.com/getting-started/install)
- [Anchor 0.30.1](https://www.anchor-lang.com/docs/installation)

Required versions are defined in [./svm/rust-toolchain.toml](./svm/rust-toolchain.toml) and [./svm/Anchor.toml](./svm/Anchor.toml)

## Recommended VSCode Settings

Recommended VSCode settings and extensions have been included as workspace settings in this repository (`.vscode`).

This includes:

- Foundry's [forge formatting](https://book.getfoundry.sh/config/vscode#3-formatter)
- [Prettier](https://marketplace.visualstudio.com/items?itemName=esbenp.prettier-vscode)
  - This should work after running `npm ci` at the root of this repo.

Additional, related settings may be required based on your use.

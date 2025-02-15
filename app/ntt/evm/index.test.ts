import { expect, test } from "bun:test";
import { sepolia } from "viem/chains";
import { evmNttHandler } from ".";

const sepoliaChainInfo = {
  chainId: 10002,
  rpc: "https://ethereum-sepolia-rpc.publicnode.com",
  baseFee: 1000n,
  coingeckoId: "ethereum",
  payeeAddress: "0x",
  gasPriceDecimals: 18,
  nativeDecimals: 18,
  executorAddress: "0xB67841A38bF16EB9999dC7B6015746506e20F0aA",
  evmChain: sepolia,
};

test("getTransceivers", async () => {
  expect(
    await evmNttHandler.getTransceivers(
      sepoliaChainInfo,
      "0x06413c42e913327Bc9a08B7C1E362BAE7C0b9598",
      // 7707338n, // missing trie node
    ),
  ).toEqual([
    { address: "0x649fF7B32C2DE771043ea105c4aAb2D724497238", type: "wormhole" },
  ]);
});

test("getTransactionMessages", async () => {
  expect(
    await evmNttHandler.getTransferMessages(
      sepoliaChainInfo,
      "0xc8eaf8610b7d2d9fdf924ffb29306f82f4ac007990969716f9adaab8c00f5ae9",
      "0x06413c42e913327Bc9a08B7C1E362BAE7C0b9598", // will have to convert from bytes32 format
      "0x000000000000000000000000000000000000000000000000000000000000009e",
    ),
  ).toEqual([
    {
      address: "0x649fF7B32C2DE771043ea105c4aAb2D724497238",
      id: "10002/000000000000000000000000649fF7B32C2DE771043ea105c4aAb2D724497238/152",
      type: "wormhole",
    },
  ]);
});

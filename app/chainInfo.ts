import { privateKeyToAddress } from "viem/accounts";
import { avalancheFuji, sepolia } from "viem/chains";
import { evmHandler } from "./handlers/evm";
import { svmHandler } from "./handlers/svm";
import { ChainInfo } from "./types";
import { Handler } from "./handlers";

export const envStringRequired = (name: string): string => {
  let s = process.env[name];
  if (!s) {
    throw new Error(`${name} is required!`);
  }
  return s;
};

export const env0xStringRequired = (name: string): `0x${string}` => {
  // check hex regex?
  let s = envStringRequired(name);
  if (!s.startsWith("0x")) {
    throw new Error(`${name} must start with 0x!`);
  }
  return s as `0x${string}`;
};

const ETH_KEY = env0xStringRequired("ETH_KEY");
const ETH_PUBLIC_KEY = privateKeyToAddress(ETH_KEY);
const SOL_KEY = env0xStringRequired("SOL_KEY");
const SOL_PUBLIC_KEY = SOL_KEY;
// const SOL_PUBLIC_KEY = `0x${web3.Keypair.fromSecretKey(
//   new Uint8Array(Buffer.from(SOL_KEY.substring(2), "hex")),
// )
//   .publicKey.toBuffer()
//   .toString("hex")}`;
const QUOTER_KEY = env0xStringRequired("QUOTER_KEY");
const QUOTER_PUBLIC_KEY = privateKeyToAddress(QUOTER_KEY);
const GUARDIAN_URL = envStringRequired("GUARDIAN_URL");
const SUPPORTED_SRC_CHAINS = [1, 6, 10002];
const SUPPORTED_DST_CHAINS = [1, 6, 10002];

interface ChainInfoWithHandler extends ChainInfo {
  handler: Handler;
}

export const CHAIN_TO_INFO: {
  [id: number]: ChainInfoWithHandler;
} = {
  1: {
    rpc: "https://api.devnet.solana.com",
    handler: svmHandler,
    baseFee: 1000n,
    coingeckoId: "solana",
    payeeAddress: SOL_PUBLIC_KEY,
    executorAddress: "Ax7mtQPbNPQmghd7C3BHrMdwwmkAXBDq7kNGfXNcc7dg",
    gasPriceDecimals: 9 + 6, // microlamports
    nativeDecimals: 9,
    privateKey: SOL_KEY,
  },
  6: {
    rpc: "https://avalanche-fuji-c-chain-rpc.publicnode.com",
    handler: evmHandler,
    baseFee: 1000n,
    coingeckoId: "avalanche-2",
    payeeAddress: "0x000000000000000000000000" + ETH_PUBLIC_KEY.substring(2),
    gasPriceDecimals: 18,
    nativeDecimals: 18,
    executorAddress: "0x6bF4A0291ADE28ccBD0f9E1aF551c9218644Ab4a",
    evmChain: avalancheFuji,
    privateKey: ETH_KEY,
  },
  10002: {
    rpc: "https://ethereum-sepolia-rpc.publicnode.com",
    handler: evmHandler,
    baseFee: 1000n,
    coingeckoId: "ethereum",
    payeeAddress: "0x000000000000000000000000" + ETH_PUBLIC_KEY.substring(2),
    gasPriceDecimals: 18,
    nativeDecimals: 18,
    executorAddress: "0xB67841A38bF16EB9999dC7B6015746506e20F0aA",
    evmChain: sepolia,
    privateKey: ETH_KEY,
  },
};

import { Chain } from "viem";

export interface ChainInfo {
  chainId: number;
  rpc: string;
  baseFee: bigint;
  coingeckoId: string;
  payeeAddress: string;
  gasPriceDecimals: number;
  nativeDecimals: number;
  executorAddress: string;
  evmChain?: Chain;
  privateKey?: `0x${string}`;
}

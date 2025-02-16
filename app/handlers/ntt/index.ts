import { ChainInfo } from "../../types";

export type NttHandler = {
  getEnabledTransceivers(
    chainInfo: ChainInfo,
    address: `0x${string}`,
    blockNumber?: bigint,
  ): Promise<{ address: `0x${string}`; type: string }[]>;
  getTransferMessages(
    chainInfo: ChainInfo,
    hash: `0x${string}`,
    address: `0x${string}`,
    messageId: `0x${string}`,
  ): Promise<{ address: `0x${string}`; type: string; id: string }[]>;
};

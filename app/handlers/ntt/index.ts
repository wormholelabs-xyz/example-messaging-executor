import { NTTv1Request, RequestForExecution } from "../../requestForExecution";
import { ChainInfo } from "../../types";

export interface NttTransceiver {
  address: `0x${string}`;
  type: string;
}

export interface NttTransceiverMessageId extends NttTransceiver {
  id: string;
}

export interface NttTransceiverPayload extends NttTransceiverMessageId {
  payload: string;
}

export interface NttHandler {
  getEnabledTransceivers(
    chainInfo: ChainInfo,
    address: `0x${string}`,
    blockNumber?: bigint,
  ): Promise<NttTransceiver[]>;
  getTransferMessages(
    chainInfo: ChainInfo,
    hash: `0x${string}`,
    address: `0x${string}`,
    messageId: `0x${string}`,
  ): Promise<NttTransceiverMessageId[]>;
  relayNTTv1(
    c: ChainInfo,
    r: RequestForExecution,
    n: NTTv1Request,
    t: NttTransceiverPayload[],
  ): Promise<string[]>;
}

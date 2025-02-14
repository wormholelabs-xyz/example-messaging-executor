import { BinaryReader } from "../BinaryReader";
import {
  ModularMessageRequest,
  RequestForExecution,
  VAAv1Request,
} from "../requestForExecution";
import { ChainInfo } from "../types";

export type Handler = {
  getGasPrice: (c: ChainInfo) => Promise<bigint>;
  getRequest: (
    c: ChainInfo,
    id: BinaryReader,
  ) => Promise<RequestForExecution | null>;
  relayVAAv1(
    c: ChainInfo,
    r: RequestForExecution,
    v: VAAv1Request,
    b: string,
  ): Promise<string[]>;
  relayMM(
    c: ChainInfo,
    r: RequestForExecution,
    m: ModularMessageRequest,
  ): Promise<string[]>;
};

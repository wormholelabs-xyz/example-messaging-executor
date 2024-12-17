import { BinaryReader } from "../BinaryReader";
import { RequestForExecution } from "../requestForExecution";

export type Handler = {
  getGasPrice: (rpc: string) => Promise<bigint>;
  getRequest: (
    rpc: string,
    executorAddress: string,
    id: BinaryReader
  ) => Promise<RequestForExecution | null>;
};

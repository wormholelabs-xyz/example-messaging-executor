import { BinaryReader } from "../BinaryReader";
import { RequestForExecution } from "../requestForExecution";

export type Handler = {
  getRequest: (
    rpc: string,
    id: BinaryReader
  ) => Promise<RequestForExecution | null>;
};

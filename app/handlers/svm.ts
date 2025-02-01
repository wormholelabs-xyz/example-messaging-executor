import axios from "axios";
import bs58 from "bs58";
import { Handler } from ".";
import { BinaryReader } from "../BinaryReader";
import { SignedQuote } from "../signedQuote";
import { RequestForExecution } from "../requestForExecution";

function parseInstructionData(data: string): RequestForExecution | null {
  const reader = new BinaryReader(bs58.decode(data));
  const descriminator = reader.readHex(8);
  if (descriminator === "0x6d6b572597c07773") {
    // amount: u64,
    const amtPaid = reader.readUint64LE();
    // dst_chain: u16,
    const dstChain = reader.readUint16LE();
    // _dst_addr: [u8; 32],
    const dstAddr = reader.readHex(32);
    // _refund_addr: Pubkey,
    const refundAddr = reader.readHex(32);
    // signed_quote_bytes: Vec<u8>,
    const signedQuoteLen = reader.readUint32LE();
    const signedQuoteBytes = reader.readHex(signedQuoteLen);
    const quoterAddress = SignedQuote.from(signedQuoteBytes).quoterAddress;
    // _request_bytes: Vec<u8>,
    const requestLen = reader.readUint32LE();
    const requestBytes = reader.readHex(requestLen);
    // _relay_instructions: Vec<u8>,
    const relayInstructionsLen = reader.readUint32LE();
    const relayInstructionsBytes = reader.readHex(relayInstructionsLen);
    return {
      amtPaid,
      dstAddr,
      dstChain,
      quoterAddress,
      refundAddr,
      relayInstructionsBytes,
      requestBytes,
      signedQuoteBytes,
    };
  }
  return null;
}

export const svmHandler: Handler = {
  getGasPrice: async (rpc: string) => {
    // TODO: get priority fee
    return BigInt("1000000");
  },
  getRequest: async (
    rpc: string,
    executorAddress: string,
    id: BinaryReader,
  ) => {
    const transactionHash = bs58.encode(id.readUint8Array(64));
    // TODO: fetch with sdk / handle versioned transactions
    const response = await axios.post(rpc, {
      jsonrpc: "2.0",
      id: 1,
      method: "getTransaction",
      params: [transactionHash, "json"],
    });
    if ("Ok" in response.data?.result?.meta?.status) {
      // TODO: this code picks up the first executor instruction in a transaction
      // TODO: use account lookup code from watcher
      const accountKeys =
        response.data.result.transaction?.message?.accountKeys;
      const executorIndex = accountKeys.indexOf(executorAddress);
      if (executorIndex >= 0) {
        const topIxs = response.data.result.transaction.message.instructions;
        for (const ix of topIxs) {
          if (ix.programIdIndex === executorIndex) {
            return parseInstructionData(ix.data);
          }
        }
        const innerIxs = response.data.result.meta.innerInstructions;
        for (const inner of innerIxs) {
          for (const ix of inner.instructions) {
            if (ix.programIdIndex === executorIndex) {
              return parseInstructionData(ix.data);
            }
          }
        }
      }
    }
    return null;
  },
};

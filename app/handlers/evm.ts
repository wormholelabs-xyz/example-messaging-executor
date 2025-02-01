import axios from "axios";
import { decodeEventLog, toEventHash } from "viem";
import { Handler } from ".";
import { BinaryReader } from "../BinaryReader";

const REQUEST_FOR_EXECUTION_TOPIC = toEventHash(
  "RequestForExecution(address,uint256,uint16,bytes32,address,bytes,bytes,bytes)",
);

export const evmHandler: Handler = {
  getGasPrice: async (rpc: string) => {
    const response = await axios.post(rpc, {
      jsonrpc: "2.0",
      id: 1,
      method: "eth_gasPrice",
      params: [],
    });
    if (response.data?.result) {
      return BigInt(response.data.result);
    }
    throw new Error(`unable to determine gas price`);
  },
  getRequest: async (
    rpc: string,
    executorAddress: string,
    id: BinaryReader,
  ) => {
    // e.g. 0x4ffd22d986913d33927a392fe4319bcd2b62f3afe1c15a2c59f77fc2cc4c20a9
    const transactionHash = id.readHex(32);
    const logIndex = id.readUint256();
    const response = await axios.post(rpc, {
      jsonrpc: "2.0",
      id: 1,
      method: "eth_getTransactionReceipt",
      params: [transactionHash],
    });
    // TODO: check success?
    const logs = response?.data?.result?.logs;
    if (logs) {
      const log = logs.find(
        (log: any) => log?.logIndex && BigInt(log.logIndex) === logIndex,
      );
      if (
        log &&
        log.removed === false &&
        log.address === executorAddress.toLowerCase() &&
        log.topics.length === 2 &&
        log.topics[0] === REQUEST_FOR_EXECUTION_TOPIC
      ) {
        const {
          args: {
            quoterAddress,
            amtPaid,
            dstChain,
            dstAddr,
            refundAddr,
            signedQuote: signedQuoteBytes,
            requestBytes,
            relayInstructions: relayInstructionsBytes,
          },
        } = decodeEventLog({
          abi: [
            {
              type: "event",
              name: "RequestForExecution",
              inputs: [
                {
                  name: "quoterAddress",
                  type: "address",
                  indexed: true,
                  internalType: "address",
                },
                {
                  name: "amtPaid",
                  type: "uint256",
                  indexed: false,
                  internalType: "uint256",
                },
                {
                  name: "dstChain",
                  type: "uint16",
                  indexed: false,
                  internalType: "uint16",
                },
                {
                  name: "dstAddr",
                  type: "bytes32",
                  indexed: false,
                  internalType: "bytes32",
                },
                {
                  name: "refundAddr",
                  type: "address",
                  indexed: false,
                  internalType: "address",
                },
                {
                  name: "signedQuote",
                  type: "bytes",
                  indexed: false,
                  internalType: "bytes",
                },
                {
                  name: "requestBytes",
                  type: "bytes",
                  indexed: false,
                  internalType: "bytes",
                },
                {
                  name: "relayInstructions",
                  type: "bytes",
                  indexed: false,
                  internalType: "bytes",
                },
              ],
              anonymous: false,
            },
          ],
          topics: log.topics,
          data: log.data,
        });
        return {
          amtPaid,
          dstAddr,
          dstChain: Number(dstChain),
          quoterAddress,
          refundAddr,
          signedQuoteBytes,
          requestBytes,
          relayInstructionsBytes,
          timestamp: new Date(Number(BigInt(log.blockTimestamp) * 1000n)),
        };
      }
    }
    return null;
  },
};

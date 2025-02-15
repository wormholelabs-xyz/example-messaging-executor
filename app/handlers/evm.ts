import axios from "axios";
import {
  createPublicClient,
  createWalletClient,
  decodeEventLog,
  http,
  toEventHash,
} from "viem";
import { Handler } from ".";
import { BinaryReader } from "../BinaryReader";
import { privateKeyToAccount } from "viem/accounts";
import { ChainInfo } from "../types";
import {
  ModularMessageRequest,
  RequestForExecution,
  VAAv1Request,
} from "../requestForExecution";
import {
  decodeRelayInstructions,
  totalGasLimitAndMsgValue,
} from "../relayInstructions";

const REQUEST_FOR_EXECUTION_TOPIC = toEventHash(
  "RequestForExecution(address,uint256,uint16,bytes32,address,bytes,bytes,bytes)",
);

async function getRequestTimestamp(log: any, rpc: string, blockHash: string) {
  if (log.blockTimestamp) {
    return new Date(Number(BigInt(log.blockTimestamp) * 1000n));
  }
  const response = await axios.post(rpc, {
    jsonrpc: "2.0",
    id: 1,
    method: "eth_getBlockByHash",
    params: [blockHash, false],
  });
  if (!response.data?.result?.timestamp) {
    throw new Error(`Unable to fetch block timestamp for ${blockHash}`);
  }
  return new Date(Number(BigInt(response.data.result.timestamp) * 1000n));
}

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
          timestamp: await getRequestTimestamp(
            log,
            rpc,
            response.data.result.blockHash,
          ),
        };
      }
    }
    return null;
  },
  relayVAAv1: async (
    c: ChainInfo,
    r: RequestForExecution,
    v: VAAv1Request,
    b: string,
  ) => {
    if (!c.privateKey) {
      throw new Error(`No private key configured`);
    }
    const account = privateKeyToAccount(c.privateKey);
    const publicClient = createPublicClient({
      chain: c.evmChain,
      transport: http(c.rpc),
    });
    const relayInstructions = decodeRelayInstructions(r.relayInstructionsBytes);
    const { gasLimit, msgValue } = totalGasLimitAndMsgValue(relayInstructions);
    const { request } = await publicClient.simulateContract({
      account,
      address: `0x${r.dstAddr.substring(26)}`,
      gas: gasLimit,
      value: msgValue,
      abi: [
        {
          type: "function",
          name: "receiveMessage",
          inputs: [{ type: "bytes" }],
          outputs: [],
        },
      ],
      functionName: "receiveMessage",
      args: [`0x${Buffer.from(b, "base64").toString("hex")}`],
    });
    const client = createWalletClient({
      account,
      chain: c.evmChain,
      transport: http(c.rpc),
    });
    return [await client.writeContract(request)];
  },
  relayMM: async (
    c: ChainInfo,
    r: RequestForExecution,
    m: ModularMessageRequest,
  ) => {
    if (!c.privateKey) {
      throw new Error(`No private key configured`);
    }
    const account = privateKeyToAccount(c.privateKey);
    const publicClient = createPublicClient({
      chain: c.evmChain,
      transport: http(c.rpc),
    });
    const relayInstructions = decodeRelayInstructions(r.relayInstructionsBytes);
    const { gasLimit, msgValue } = totalGasLimitAndMsgValue(relayInstructions);
    // TODO: call `isReady` first before attempting relay
    // or just only try for a certain number of times / up to a certain timeout
    // which must be longer than a typical finalized message
    const { request } = await publicClient.simulateContract({
      account,
      address: `0x${r.dstAddr.substring(26)}`,
      gas: gasLimit,
      value: msgValue,
      abi: [
        {
          type: "function",
          name: "executeMsg",
          inputs: [
            { type: "uint16" },
            { type: "bytes32" },
            { type: "uint64" },
            { type: "bytes" },
          ],
          outputs: [],
        },
      ],
      functionName: "executeMsg",
      args: [m.chain, m.address, m.sequence, m.payload],
    });
    const walletClient = createWalletClient({
      account,
      chain: c.evmChain,
      transport: http(c.rpc),
    });
    return [await walletClient.writeContract(request)];
  },
};

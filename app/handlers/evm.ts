import axios from "axios";
import { decodeEventLog } from "viem";
import { Handler } from ".";
import { BinaryReader } from "../BinaryReader";

const REQUEST_FOR_EXECUTION_TOPIC =
  "0x48f75baf726bbb12e77ca4f0f39aeed4ec43e710a7bdd132adbe1645ee90e0a2";
// TODO: get this from config
const EXECUTOR_ADDRESS =
  "0x634fACff0663E8da9e9Eae4963d2F5006078b7BD".toLowerCase();

// event RequestForExecution(
//     address indexed quoterAddress,
//     uint256 amtPaid,
//     uint16 dstChain,
//     bytes32 dstAddr,
//     uint256 gasLimit,
//     uint256 msgValue,
//     address refundAddr,
//     bytes signedQuote,
//     bytes requestBytes
// );

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
  getRequest: async (rpc: string, id: BinaryReader) => {
    // e.g. 0x4ffd22d986913d33927a392fe4319bcd2b62f3afe1c15a2c59f77fc2cc4c20a9
    const transactionHash = id.readHex(32);
    const logIndex = id.readUint256();
    const response = await axios.post(rpc, {
      jsonrpc: "2.0",
      id: 1,
      method: "eth_getTransactionReceipt",
      params: [transactionHash],
    });
    const logs = response?.data?.result?.logs;
    if (logs) {
      const log = logs.find(
        (log: any) => log?.logIndex && BigInt(log.logIndex) === logIndex
      );
      if (
        log &&
        log.removed === false &&
        log.address === EXECUTOR_ADDRESS &&
        log.topics.length === 2 &&
        log.topics[0] === REQUEST_FOR_EXECUTION_TOPIC
      ) {
        const {
          args: {
            quoterAddress,
            amtPaid,
            dstChain,
            dstAddr,
            gasLimit,
            msgValue,
            refundAddr,
            signedQuote: signedQuoteBytes,
            requestBytes,
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
                  name: "gasLimit",
                  type: "uint256",
                  indexed: false,
                  internalType: "uint256",
                },
                {
                  name: "msgValue",
                  type: "uint256",
                  indexed: false,
                  internalType: "uint256",
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
          gasLimit,
          msgValue,
          quoterAddress,
          refundAddr,
          requestBytes,
          signedQuoteBytes,
          timestamp: new Date(Number(BigInt(log.blockTimestamp) * 1000n)),
        };
      }
    }
    return null;
  },
};

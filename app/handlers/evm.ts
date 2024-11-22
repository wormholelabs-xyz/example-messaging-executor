import axios from "axios";
import { decodeParameters } from "web3-eth-abi";
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
        const { 0: quoterAddress } = decodeParameters(
          ["address"],
          log.topics[1]
        ) as any;
        const {
          0: amtPaid,
          1: dstChain,
          2: dstAddr,
          3: gasLimit,
          4: msgValue,
          5: refundAddr,
          6: signedQuoteBytes,
          7: requestBytes,
        } = decodeParameters(
          [
            "uint256",
            "uint16",
            "bytes32",
            "uint256",
            "uint256",
            "address",
            "bytes",
            "bytes",
          ],
          log.data
        ) as any;
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

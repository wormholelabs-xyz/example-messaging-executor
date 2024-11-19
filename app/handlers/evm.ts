import axios from "axios";
import { decodeParameters } from "web3-eth-abi";
import { Handler } from ".";
import { BinaryReader } from "../BinaryReader";

const REQUEST_FOR_EXECUTION_TOPIC =
  "0x48f75baf726bbb12e77ca4f0f39aeed4ec43e710a7bdd132adbe1645ee90e0a2";
// TODO: get this from config
const EXECUTOR_ADDRESS = "0x634facff0663e8da9e9eae4963d2f5006078b7bd";

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
        const quoterAddress = `0x${log.topics[1].substring(26)}`;
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
          log.data.slice(2)
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
        };
      }
    }
    return null;
  },
};

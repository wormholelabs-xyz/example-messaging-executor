import { BinaryWriter } from "./BinaryWriter";
import { privateKeyToAddress, sign, signRaw } from "web3-eth-accounts";

// struct Signature {
//     bytes32 r;
//     bytes32 s;
//     uint8 v;
// }

// struct SignedQuote {
//     bytes4 prefix;
//     address quoterAddress;
//     bytes32 payeeAddress;
//     uint16 srcChain;
//     uint16 dstChain;
//     uint64 baseFee;
//     uint64 conversionRate;
//     uint64 expiryTime;
//     Signature signature;
// }

export type SignedQuote = {
  prefix: string;
  quoterAddress: string;
  payeeAddress: string;
  srcChain: number;
  dstChain: number;
  expiryTime: bigint;
  baseFee: bigint;
  conversionRate: bigint;
  signature: string;
};

export function makeSignedQuote(
  privateKey: string,
  payeeAddress: string,
  srcChain: number,
  dstChain: number,
  baseFee: bigint,
  conversionRate: bigint,
  expiryTime: Date
) {
  if (payeeAddress.length !== 64) {
    throw new Error("invalid payeeAddress length");
  }
  const address = privateKeyToAddress(privateKey);
  const writer = new BinaryWriter()
    .writeUint8Array(Buffer.from("EQ01"))
    .writeUint8Array(Buffer.from(address.substring(2), "hex"))
    .writeUint8Array(Buffer.from(payeeAddress, "hex"))
    .writeUint16(srcChain)
    .writeUint16(dstChain)
    .writeUint64(BigInt(expiryTime.getTime()) / BigInt(1000))
    .writeUint64(baseFee)
    .writeUint64(conversionRate);
  const quote = Buffer.from(writer.data()).toString("hex");
  const result = signRaw(`0x${quote}`, privateKey);
  console.log(result);
  return quote + result.signature.substring(2);
}

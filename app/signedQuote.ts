import { recover, signRaw } from "web3-eth-accounts";
import { BinaryWriter } from "./BinaryWriter";
import { BinaryReader } from "./BinaryReader";
import { sha3Raw } from "web3-utils";

function normalize(amount: bigint, from: number, to: number) {
  if (from > to) {
    return amount / 10n ** BigInt(from - to);
  } else if (from < to) {
    return amount * 10n ** BigInt(to - from);
  }
  return amount;
}

function mul(a: bigint, b: bigint, decimals: number) {
  return (a * b) / 10n ** BigInt(decimals);
}
function div(a: bigint, b: bigint, decimals: number) {
  return (a * 10n ** BigInt(decimals)) / b;
}

export class SignedQuote {
  static prefix = "EQ01";
  static byteLength = 4 + 20 + 32 + 2 + 2 + 8 + 8 + 8 + 8 + 8 + 65;
  static decimals = 10;
  quoterAddress: string;
  payeeAddress: string;
  srcChain: number;
  dstChain: number;
  expiryTime: Date;
  baseFee: bigint;
  dstGasPrice: bigint;
  srcPrice: bigint;
  dstPrice: bigint;
  signature: string | undefined;

  constructor(
    quoterAddress: string,
    payeeAddress: string,
    srcChain: number,
    dstChain: number,
    expiryTime: Date,
    baseFee: bigint,
    dstGasPrice: bigint,
    srcPrice: bigint,
    dstPrice: bigint,
    signature?: string
  ) {
    if (payeeAddress.replace("0x", "").length !== 64) {
      throw new Error("invalid payeeAddress length");
    }
    this.quoterAddress = quoterAddress;
    this.payeeAddress = payeeAddress;
    this.srcChain = srcChain;
    this.dstChain = dstChain;
    this.expiryTime = expiryTime;
    this.baseFee = baseFee;
    this.dstGasPrice = dstGasPrice;
    this.srcPrice = srcPrice;
    this.dstPrice = dstPrice;
    this.signature = signature;
  }

  static from(bytes: string): SignedQuote {
    const reader = new BinaryReader(bytes);
    if (reader.length() !== SignedQuote.byteLength) {
      throw new Error("invalid quote length");
    }
    const prefix = reader.readString(4);
    if (prefix !== SignedQuote.prefix) {
      throw new Error("invalid quote prefix");
    }
    return new SignedQuote(
      reader.readHex(20),
      reader.readHex(32),
      reader.readUint16(),
      reader.readUint16(),
      new Date(Number(reader.readUint64() * 1000n)),
      reader.readUint64(),
      reader.readUint64(),
      reader.readUint64(),
      reader.readUint64(),
      reader.readHex(65)
    );
  }

  serializeBody(): string {
    return new BinaryWriter()
      .writeUint8Array(Buffer.from(SignedQuote.prefix))
      .writeHex(this.quoterAddress)
      .writeHex(this.payeeAddress)
      .writeUint16(this.srcChain)
      .writeUint16(this.dstChain)
      .writeUint64(BigInt(this.expiryTime.getTime()) / 1000n)
      .writeUint64(this.baseFee)
      .writeUint64(this.dstGasPrice)
      .writeUint64(this.srcPrice)
      .writeUint64(this.dstPrice)
      .toHex();
  }

  sign(privateKey: string) {
    const serialized = this.serializeBody();
    const result = signRaw(serialized, privateKey);
    this.signature = result.signature;
    return serialized + this.signature.substring(2);
  }

  verify(allowedQuoterAddresses: string[]) {
    if (!allowedQuoterAddresses.includes(this.quoterAddress)) {
      `Bad quoterAddress. Expected one of: ${allowedQuoterAddresses}, Received: ${this.quoterAddress}`;
    }
    const recoveredPublicKey = recover(
      sha3Raw(this.serializeBody()),
      this.signature,
      true
    ).toLowerCase();
    if (recoveredPublicKey !== this.quoterAddress) {
      throw new Error(
        `Bad quote signature recovery. Expected: ${this.quoterAddress}, Received: ${recoveredPublicKey}`
      );
    }
  }

  estimate(
    gasLimit: bigint,
    msgValue: bigint,
    dstGasPriceDecimals: number,
    srcTokenDecimals: number,
    dstNativeDecimals: number
  ): bigint {
    // TODO: add baseFee
    // TODO: add msgValue
    const r = 18; // decimal resolution
    const nGasLimitCost = normalize(
      gasLimit * this.dstGasPrice,
      dstGasPriceDecimals,
      r
    );
    const nSrcPrice = normalize(this.srcPrice, SignedQuote.decimals, r);
    const nDstPrice = normalize(this.dstPrice, SignedQuote.decimals, r);
    return normalize(
      mul(nGasLimitCost, div(nDstPrice, nSrcPrice, r), r),
      r,
      srcTokenDecimals
    );
  }
}

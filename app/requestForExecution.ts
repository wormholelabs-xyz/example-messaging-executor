import { BinaryReader, hexToUint8Array } from "./BinaryReader";
import { BinaryWriter } from "./BinaryWriter";

export type RequestForExecution = {
  quoterAddress: string;
  amtPaid: bigint;
  dstChain: number;
  dstAddr: string;
  gasLimit: bigint;
  msgValue: bigint;
  refundAddr: string;
  signedQuoteBytes: string;
  requestBytes: string;
};

export class ModularMessageRequest {
  static prefix = "ERM1";
  static byteLength = 4 + 2 + 32 + 8 + 4;
  chain: number;
  address: string;
  sequence: bigint;
  payload: string;

  constructor(
    chain: number,
    address: string,
    sequence: bigint,
    payload: string
  ) {
    if (address.replace("0x", "").length !== 64) {
      throw new Error("invalid address length");
    }
    this.chain = chain;
    this.address = address;
    this.sequence = sequence;
    this.payload = payload;
  }

  static from(bytes: string): ModularMessageRequest {
    const reader = new BinaryReader(bytes);
    if (reader.length() < ModularMessageRequest.byteLength) {
      throw new Error("invalid request length");
    }
    const prefix = reader.readString(4);
    if (prefix !== ModularMessageRequest.prefix) {
      throw new Error("invalid request prefix");
    }
    const chain = reader.readUint16();
    const address = reader.readHex(32);
    const sequence = reader.readUint64();
    const payloadLen = reader.readUint32();
    if (reader.length() !== ModularMessageRequest.byteLength + payloadLen) {
      throw new Error("invalid request payload length");
    }
    const payload = reader.readHex(payloadLen);
    return new ModularMessageRequest(chain, address, sequence, payload);
  }

  serialize(): string {
    const payload = hexToUint8Array(this.payload);
    return new BinaryWriter()
      .writeUint8Array(Buffer.from(ModularMessageRequest.prefix))
      .writeUint16(this.chain)
      .writeHex(this.address)
      .writeUint64(this.sequence)
      .writeUint32(payload.length)
      .writeUint8Array(payload)
      .toHex();
  }
}

export class VAAv1Request {
  static prefix = "ERV1";
  static byteLength = 4 + 2 + 32 + 8;
  chain: number;
  address: string;
  sequence: bigint;

  constructor(chain: number, address: string, sequence: bigint) {
    if (address.replace("0x", "").length !== 64) {
      throw new Error("invalid address length");
    }
    this.chain = chain;
    this.address = address;
    this.sequence = sequence;
  }

  static from(bytes: string): VAAv1Request {
    const reader = new BinaryReader(bytes);
    if (reader.length() !== VAAv1Request.byteLength) {
      throw new Error("invalid request length");
    }
    const prefix = reader.readString(4);
    if (prefix !== VAAv1Request.prefix) {
      throw new Error("invalid request prefix");
    }
    return new VAAv1Request(
      reader.readUint16(),
      reader.readHex(32),
      reader.readUint64()
    );
  }

  serialize(): string {
    return new BinaryWriter()
      .writeUint8Array(Buffer.from(VAAv1Request.prefix))
      .writeUint16(this.chain)
      .writeHex(this.address)
      .writeUint64(this.sequence)
      .toHex();
  }
}
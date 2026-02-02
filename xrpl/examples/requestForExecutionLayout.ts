import type { CustomConversion, DeriveType, Layout } from "binary-layout";
import { deserialize, serialize } from "binary-layout";
import { fromBytes, fromHex, toHex } from "viem";
import {
  signedQuoteLayout,
  relayInstructionsLayout,
} from "@wormhole-foundation/sdk-connect";

export const hexConversion = {
  to: (encoded: Uint8Array) => fromBytes(encoded, "hex"),
  from: (decoded: `0x${string}`) => fromHex(decoded, "bytes"),
} as const satisfies CustomConversion<Uint8Array, `0x${string}`>;

export enum RequestPrefix {
  ERN1 = "ERN1", // NTT_V1
}

export const nttV1RequestLayout = [
  { name: "srcChain", binary: "uint", size: 2 },
  {
    name: "srcManager",
    binary: "bytes",
    size: 32,
    custom: hexConversion,
  },
  {
    name: "messageId",
    binary: "bytes",
    size: 32,
    custom: hexConversion,
  },
] as const satisfies Layout;

export type NTTv1Request = DeriveType<typeof nttV1RequestLayout>;

export const requestLayout = [
  {
    name: "request",
    binary: "switch",
    idSize: 4,
    idTag: "prefix",
    layouts: [[[0x45524e31, RequestPrefix.ERN1], nttV1RequestLayout]],
  },
] as const satisfies Layout;

export type RequestLayout = DeriveType<typeof requestLayout>;

export function deserializeRequest(requestBytes: `0x${string}`): RequestLayout {
  return deserialize(requestLayout, fromHex(requestBytes, "bytes"));
}

export function serializeRequest(instruction: RequestLayout): `0x${string}` {
  return toHex(serialize(requestLayout, instruction));
}

export const REQUEST_FOR_EXECUTION_VERSION_0 = 0;

const requestForExecutionV0Layout = [
  { name: "dstChain", binary: "uint", size: 2 },
  {
    name: "dstAddr",
    binary: "bytes",
    size: 32,
    custom: hexConversion,
  },
  {
    name: "refundAddr",
    binary: "bytes",
    size: 20,
    custom: hexConversion,
  },
  {
    name: "signedQuote",
    binary: "bytes",
    lengthSize: 2,
    layout: signedQuoteLayout,
  },
  {
    name: "requestBytes",
    binary: "bytes",
    lengthSize: 2,
    layout: requestLayout,
  },
  {
    name: "relayInstructions",
    binary: "bytes",
    lengthSize: 2,
    layout: relayInstructionsLayout,
  },
] as const satisfies Layout;

export const requestForExecutionLayout = [
  {
    name: "payload",
    binary: "switch",
    idSize: 1,
    idTag: "version",
    layouts: [[[REQUEST_FOR_EXECUTION_VERSION_0, 0], requestForExecutionV0Layout]],
  },
] as const satisfies Layout;

export type RequestForExecution = DeriveType<typeof requestForExecutionLayout>;

export function deserializeRequestForExecution(
  data: `0x${string}`,
): RequestForExecution {
  return deserialize(requestForExecutionLayout, fromHex(data, "bytes"));
}

export function serializeRequestForExecution(
  request: RequestForExecution,
): `0x${string}` {
  return toHex(serialize(requestForExecutionLayout, request));
}

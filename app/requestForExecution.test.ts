import { expect, test } from "bun:test";
import {
  ModularMessageRequest,
  NTTv1Request,
  VAAv1Request,
} from "./requestForExecution";

// Serialize BigInts as strings in responses
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore: Unreachable code error
BigInt.prototype.toJSON = function () {
  return this.toString();
};

const mockManager =
  "0x1234123412341234123412341234123412341234123412341234123412341234";
const mockMessageId =
  "0x4567456745674567456745674567456745674567456745674567456745674567";

test("ModularMessageRequest", () => {
  const payload = Buffer.from("hello world").toString("hex");
  const r = new ModularMessageRequest(1, mockManager, 42n, payload);
  expect(r.chain).toBe(1);
  expect(r.address).toBe(mockManager);
  expect(r.sequence).toBe(42n);
  expect(r.payload).toBe(payload);
  const ser = r.serialize();
  expect(ser.length).toBe(
    ModularMessageRequest.byteLength * 2 + 2 + payload.length,
  );
  expect(ser).toBe(
    "0x45524d3100011234123412341234123412341234123412341234123412341234123412341234000000000000002a0000000b68656c6c6f20776f726c64",
  );
  expect(ModularMessageRequest.from(ser).serialize()).toBe(ser);
  expect(JSON.stringify(r)).toInclude('prefix":"ERM1"');
});

test("VAAv1Request", () => {
  const r = new VAAv1Request(1, mockManager, 42n);
  expect(r.chain).toBe(1);
  expect(r.address).toBe(mockManager);
  expect(r.sequence).toBe(42n);
  const ser = r.serialize();
  expect(ser.length).toBe(VAAv1Request.byteLength * 2 + 2);
  expect(ser).toBe(
    "0x4552563100011234123412341234123412341234123412341234123412341234123412341234000000000000002a",
  );
  expect(VAAv1Request.from(ser).serialize()).toBe(ser);
  expect(JSON.stringify(r)).toInclude('prefix":"ERV1"');
});

test("NTTv1Request", () => {
  const r = new NTTv1Request(1, mockManager, mockMessageId);
  expect(r.srcChain).toBe(1);
  expect(r.srcManager).toBe(mockManager);
  expect(r.messageId).toBe(mockMessageId);
  const ser = r.serialize();
  expect(ser.length).toBe(NTTv1Request.byteLength * 2 + 2);
  expect(ser).toBe(
    "0x45524e31000112341234123412341234123412341234123412341234123412341234123412344567456745674567456745674567456745674567456745674567456745674567",
  );
  expect(NTTv1Request.from(ser).serialize()).toBe(ser);
  expect(JSON.stringify(r)).toInclude('prefix":"ERN1"');
});

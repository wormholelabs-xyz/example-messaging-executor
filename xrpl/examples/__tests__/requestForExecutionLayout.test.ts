import { describe, it, expect } from "bun:test";
import {
  serializeRequestForExecution,
  deserializeRequestForExecution,
  type RequestForExecution,
  RequestPrefix,
  REQUEST_FOR_EXECUTION_VERSION_0,
} from "../requestForExecutionLayout";

describe("RequestForExecution Layout", () => {
  const createMockRequest = (): RequestForExecution => {
    const dummyQuoterAddressBytes = new Uint8Array(20).fill(0x12);
    const dummyPayeeAddress = new Uint8Array(32).fill(0xab);
    const dummySignature = new Uint8Array(65).fill(0xcd);

    return {
      payload: {
        version: 0,
        dstChain: 1, // Solana
        dstAddr:
          "0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" as `0x${string}`,
        refundAddr:
          "0xaabbccddaabbccddaabbccddaabbccddaabbccdd" as `0x${string}`,
        signedQuote: {
          quote: {
            prefix: "EQ01",
            quoterAddress: dummyQuoterAddressBytes,
            payeeAddress: dummyPayeeAddress,
            srcChain: 66, // XRPL
            dstChain: 1, // Solana
            expiryTime: new Date("2026-12-31T23:59:59Z"),
            baseFee: 1000000n,
            dstGasPrice: 20000000000n,
            srcPrice: 100000000n,
            dstPrice: 200000000n,
          },
          signature: dummySignature,
        },
        requestBytes: {
          request: {
            prefix: RequestPrefix.ERN1,
            srcChain: 1,
            srcManager:
              "0x0000000000000000000000001111111111111111111111111111111111111111" as `0x${string}`,
            messageId:
              "0x0000000000000000000000002222222222222222222222222222222222222222" as `0x${string}`,
          },
        },
        relayInstructions: {
          requests: [
            {
              request: {
                type: "GasInstruction",
                gasLimit: 200000n,
                msgValue: 0n,
              },
            },
          ],
        },
      },
    };
  };

  describe("Serialization", () => {
    it("should serialize a RequestForExecution to hex", () => {
      const request = createMockRequest();
      const serialized = serializeRequestForExecution(request);

      expect(serialized).toMatch(/^0x[0-9a-f]+$/i);
      expect(serialized.length).toBeGreaterThan(2); // More than just "0x"
    });

    it("should start with version byte 0", () => {
      const request = createMockRequest();
      const serialized = serializeRequestForExecution(request);

      // First byte after 0x should be 00 (version 0)
      const versionByte = serialized.slice(2, 4);
      expect(versionByte).toBe("00");
    });

    it("should include dstChain in correct position", () => {
      const request = createMockRequest();
      const serialized = serializeRequestForExecution(request);

      // Version (1 byte) + dstChain (2 bytes big-endian)
      // dstChain = 1 = 0x0001
      const dstChainBytes = serialized.slice(2 + 2, 2 + 2 + 4); // Skip "0x" and version byte
      expect(dstChainBytes).toBe("0001");
    });
  });

  describe("Deserialization", () => {
    it("should deserialize a serialized RequestForExecution", () => {
      const originalRequest = createMockRequest();
      const serialized = serializeRequestForExecution(originalRequest);
      const deserialized = deserializeRequestForExecution(serialized);

      expect(deserialized.payload.version).toBe(0);
      expect(deserialized.payload.dstChain).toBe(
        originalRequest.payload.dstChain,
      );
      expect(deserialized.payload.dstAddr).toBe(
        originalRequest.payload.dstAddr,
      );
      expect(deserialized.payload.refundAddr).toBe(
        originalRequest.payload.refundAddr,
      );
    });

    it("should deserialize signedQuote correctly", () => {
      const originalRequest = createMockRequest();
      const serialized = serializeRequestForExecution(originalRequest);
      const deserialized = deserializeRequestForExecution(serialized);

      expect(deserialized.payload.signedQuote.quote.prefix).toBe("EQ01");
      expect(deserialized.payload.signedQuote.quote.srcChain).toBe(66);
      expect(deserialized.payload.signedQuote.quote.dstChain).toBe(1);
      expect(deserialized.payload.signedQuote.quote.baseFee).toBe(1000000n);
      expect(deserialized.payload.signedQuote.quote.dstGasPrice).toBe(
        20000000000n,
      );
    });

    it("should deserialize requestBytes correctly", () => {
      const originalRequest = createMockRequest();
      const serialized = serializeRequestForExecution(originalRequest);
      const deserialized = deserializeRequestForExecution(serialized);

      expect(deserialized.payload.requestBytes.request.prefix).toBe(
        RequestPrefix.ERN1,
      );
      expect(deserialized.payload.requestBytes.request.srcChain).toBe(1);
      expect(deserialized.payload.requestBytes.request.srcManager).toBe(
        originalRequest.payload.requestBytes.request.srcManager,
      );
      expect(deserialized.payload.requestBytes.request.messageId).toBe(
        originalRequest.payload.requestBytes.request.messageId,
      );
    });

    it("should deserialize relayInstructions correctly", () => {
      const originalRequest = createMockRequest();
      const serialized = serializeRequestForExecution(originalRequest);
      const deserialized = deserializeRequestForExecution(serialized);

      expect(deserialized.payload.relayInstructions.requests).toHaveLength(1);
      const instruction =
        deserialized.payload.relayInstructions.requests[0].request;
      expect(instruction.type).toBe("GasInstruction");
      if (instruction.type === "GasInstruction") {
        expect(instruction.gasLimit).toBe(200000n);
        expect(instruction.msgValue).toBe(0n);
      }
    });

    it("should handle timestamp correctly", () => {
      const originalRequest = createMockRequest();
      const serialized = serializeRequestForExecution(originalRequest);
      const deserialized = deserializeRequestForExecution(serialized);

      const originalTime =
        originalRequest.payload.signedQuote.quote.expiryTime.getTime();
      const deserializedTime =
        deserialized.payload.signedQuote.quote.expiryTime.getTime();

      // Allow for small differences due to precision (within 1 second)
      expect(Math.abs(originalTime - deserializedTime)).toBeLessThan(1000);
    });
  });

  describe("Round-trip", () => {
    it("should maintain data integrity through serialize-deserialize cycle", () => {
      const originalRequest = createMockRequest();
      const serialized = serializeRequestForExecution(originalRequest);
      const deserialized = deserializeRequestForExecution(serialized);

      // Serialize again and compare
      const reserialized = serializeRequestForExecution(deserialized);
      expect(reserialized).toBe(serialized);
    });
  });

  describe("Version enforcement", () => {
    it("should only accept version 0", () => {
      const request = createMockRequest();
      expect(request.payload.version).toBe(REQUEST_FOR_EXECUTION_VERSION_0);
      expect(request.payload.version).toBe(0);
    });

    it("should fail to deserialize with invalid version byte", () => {
      // Create a hex string with version byte set to 1 (invalid)
      const invalidVersionHex = "0x01" + "0001" + "00".repeat(100); // version 1 + some data

      expect(() => {
        deserializeRequestForExecution(invalidVersionHex as `0x${string}`);
      }).toThrow();
    });
  });

  describe("Edge cases", () => {
    it("should handle maximum values for bigints", () => {
      const dummyQuoterAddressBytes = new Uint8Array(20).fill(0x12);
      const dummyPayeeAddress = new Uint8Array(32).fill(0xab);
      const dummySignature = new Uint8Array(65).fill(0xcd);

      const request: RequestForExecution = {
        payload: {
          version: 0,
          dstChain: 1,
          dstAddr:
            "0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" as `0x${string}`,
          refundAddr:
            "0xaabbccddaabbccddaabbccddaabbccddaabbccdd" as `0x${string}`,
          signedQuote: {
            quote: {
              prefix: "EQ01",
              quoterAddress: dummyQuoterAddressBytes,
              payeeAddress: dummyPayeeAddress,
              srcChain: 66,
              dstChain: 1,
              expiryTime: new Date("2026-12-31T23:59:59Z"),
              baseFee: 18446744073709551615n, // Max u64
              dstGasPrice: 18446744073709551615n, // Max u64
              srcPrice: 18446744073709551615n, // Max u64
              dstPrice: 18446744073709551615n, // Max u64
            },
            signature: dummySignature,
          },
          requestBytes: {
            request: {
              prefix: RequestPrefix.ERN1,
              srcChain: 1,
              srcManager:
                "0x0000000000000000000000001111111111111111111111111111111111111111" as `0x${string}`,
              messageId:
                "0x0000000000000000000000002222222222222222222222222222222222222222" as `0x${string}`,
            },
          },
          relayInstructions: {
            requests: [
              {
                request: {
                  type: "GasInstruction",
                  gasLimit: 340282366920938463463374607431768211455n, // Max u128
                  msgValue: 340282366920938463463374607431768211455n, // Max u128
                },
              },
            ],
          },
        },
      };

      const serialized = serializeRequestForExecution(request);
      const deserialized = deserializeRequestForExecution(serialized);

      expect(deserialized.payload.signedQuote.quote.baseFee).toBe(
        18446744073709551615n,
      );
      const gasInstruction =
        deserialized.payload.relayInstructions.requests[0].request;
      if (gasInstruction.type === "GasInstruction") {
        expect(gasInstruction.gasLimit).toBe(
          340282366920938463463374607431768211455n,
        );
      }
    });

    it("should handle empty relay instructions array", () => {
      const dummyQuoterAddressBytes = new Uint8Array(20).fill(0x12);
      const dummyPayeeAddress = new Uint8Array(32).fill(0xab);
      const dummySignature = new Uint8Array(65).fill(0xcd);

      const request: RequestForExecution = {
        payload: {
          version: 0,
          dstChain: 1,
          dstAddr:
            "0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" as `0x${string}`,
          refundAddr:
            "0xaabbccddaabbccddaabbccddaabbccddaabbccdd" as `0x${string}`,
          signedQuote: {
            quote: {
              prefix: "EQ01",
              quoterAddress: dummyQuoterAddressBytes,
              payeeAddress: dummyPayeeAddress,
              srcChain: 66,
              dstChain: 1,
              expiryTime: new Date("2026-12-31T23:59:59Z"),
              baseFee: 1000000n,
              dstGasPrice: 20000000000n,
              srcPrice: 100000000n,
              dstPrice: 200000000n,
            },
            signature: dummySignature,
          },
          requestBytes: {
            request: {
              prefix: RequestPrefix.ERN1,
              srcChain: 1,
              srcManager:
                "0x0000000000000000000000001111111111111111111111111111111111111111" as `0x${string}`,
              messageId:
                "0x0000000000000000000000002222222222222222222222222222222222222222" as `0x${string}`,
            },
          },
          relayInstructions: {
            requests: [],
          },
        },
      };

      const serialized = serializeRequestForExecution(request);
      const deserialized = deserializeRequestForExecution(serialized);

      expect(deserialized.payload.relayInstructions.requests).toHaveLength(0);
    });

    it("should handle multiple relay instructions", () => {
      const dummyQuoterAddressBytes = new Uint8Array(20).fill(0x12);
      const dummyPayeeAddress = new Uint8Array(32).fill(0xab);
      const dummySignature = new Uint8Array(65).fill(0xcd);

      const request: RequestForExecution = {
        payload: {
          version: 0,
          dstChain: 1,
          dstAddr:
            "0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" as `0x${string}`,
          refundAddr:
            "0xaabbccddaabbccddaabbccddaabbccddaabbccdd" as `0x${string}`,
          signedQuote: {
            quote: {
              prefix: "EQ01",
              quoterAddress: dummyQuoterAddressBytes,
              payeeAddress: dummyPayeeAddress,
              srcChain: 66,
              dstChain: 1,
              expiryTime: new Date("2026-12-31T23:59:59Z"),
              baseFee: 1000000n,
              dstGasPrice: 20000000000n,
              srcPrice: 100000000n,
              dstPrice: 200000000n,
            },
            signature: dummySignature,
          },
          requestBytes: {
            request: {
              prefix: RequestPrefix.ERN1,
              srcChain: 1,
              srcManager:
                "0x0000000000000000000000001111111111111111111111111111111111111111" as `0x${string}`,
              messageId:
                "0x0000000000000000000000002222222222222222222222222222222222222222" as `0x${string}`,
            },
          },
          relayInstructions: {
            requests: [
              {
                request: {
                  type: "GasInstruction",
                  gasLimit: 200000n,
                  msgValue: 0n,
                },
              },
              {
                request: {
                  type: "GasInstruction",
                  gasLimit: 300000n,
                  msgValue: 1000000n,
                },
              },
            ],
          },
        },
      };

      const serialized = serializeRequestForExecution(request);
      const deserialized = deserializeRequestForExecution(serialized);

      expect(deserialized.payload.relayInstructions.requests).toHaveLength(2);
      const secondInstruction =
        deserialized.payload.relayInstructions.requests[1].request;
      expect(secondInstruction.type).toBe("GasInstruction");
      if (secondInstruction.type === "GasInstruction") {
        expect(secondInstruction.gasLimit).toBe(300000n);
        expect(secondInstruction.msgValue).toBe(1000000n);
      }
    });
  });
});

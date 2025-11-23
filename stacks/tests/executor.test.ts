
import { describe, expect, it } from "vitest";
import { Cl } from "@stacks/transactions";

const accounts = simnet.getAccounts();
const address1 = accounts.get("wallet_1")!;

// Helper function to create a signed quote buffer
function createSignedQuoteBuffer(options: {
  srcChain: number;
  dstChain: number;
  expiryTime: number;
  quoterAddr?: Uint8Array;    // 20-byte EVM address at offset 4
  payeeAddr?: Uint8Array;     // 32-byte universal address at offset 24
}): Uint8Array {
  const buffer = new Uint8Array(8192);
  
  // Offset 4-24: Quoter address (20 bytes)
  if (options.quoterAddr) {
    buffer.set(options.quoterAddr, 4);
  } else {
    // Default quoter address (20 bytes)
    buffer.set(new Uint8Array([0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 
                              0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00, 0x11, 0x22, 0x33, 0x44]), 4);
  }
  
  // Offset 24-56: Payee address (32 bytes) 
  if (options.payeeAddr) {
    buffer.set(options.payeeAddr, 24);
  } else {
    // Default payee address (32 bytes of test data)
    const defaultPayee = new Uint8Array(32);
    defaultPayee.fill(0xAB);
    buffer.set(defaultPayee, 24);
  }
  
  // Offset 56-58: Source chain (uint16 big-endian)
  buffer[56] = (options.srcChain >> 8) & 0xFF;
  buffer[57] = options.srcChain & 0xFF;
  
  // Offset 58-60: Destination chain (uint16 big-endian) 
  buffer[58] = (options.dstChain >> 8) & 0xFF;
  buffer[59] = options.dstChain & 0xFF;
  
  // Offset 60-68: Expiry time (uint64 big-endian)
  const expiry = BigInt(options.expiryTime);
  for (let i = 0; i < 8; i++) {
    buffer[60 + i] = Number((expiry >> BigInt(8 * (7 - i))) & 0xFFn);
  }
  
  return buffer;
}

describe("Executor Contract Tests", () => {
  it("ensures simnet is well initialised", () => {
    expect(simnet.blockHeight).toBeDefined();
  });

  describe("validate-quote-header", () => {
    it("should validate a correct quote header", () => {
      // Create a valid quote buffer
      const validQuote = createSignedQuoteBuffer({
        srcChain: 1,    // OUR-CHAIN = 1  
        dstChain: 2,    // Destination chain
        expiryTime: 9999999999 // Future timestamp placeholder // 1 hour in the future
      });
      
      const { result } = simnet.callReadOnlyFn(
        "executor", 
        "validate-quote-header",
        [Cl.buffer(validQuote), Cl.uint(2)], // dst-chain = 2
        address1
      );
      
      expect(result).toBeOk(Cl.bool(true));
    });

    it("should reject quote with wrong source chain", () => {
      const invalidQuote = createSignedQuoteBuffer({
        srcChain: 99,   // Wrong source chain (OUR-CHAIN = 1)
        dstChain: 2,
        expiryTime: 9999999999 // Future timestamp placeholder
      });
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "validate-quote-header", 
        [Cl.buffer(invalidQuote), Cl.uint(2)],
        address1
      );
      
      expect(result).toBeErr(Cl.uint(1001)); // ERR-QUOTE-SRC-CHAIN-MISMATCH
    });

    it("should reject quote with wrong destination chain", () => {
      const invalidQuote = createSignedQuoteBuffer({
        srcChain: 1,    // Correct source chain
        dstChain: 99,   // Wrong destination chain
        expiryTime: 9999999999 // Future timestamp placeholder
      });
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "validate-quote-header",
        [Cl.buffer(invalidQuote), Cl.uint(2)], // Requesting dst-chain = 2
        address1
      );
      
      expect(result).toBeErr(Cl.uint(1002)); // ERR-QUOTE-DST-CHAIN-MISMATCH
    });

    it("should reject expired quote", () => {
      const expiredQuote = createSignedQuoteBuffer({
        srcChain: 1,
        dstChain: 2,
        expiryTime: 1000000000 // Past timestamp placeholder // 1 hour ago (expired)
      });

      const { result } = simnet.callReadOnlyFn(
        "executor",
        "validate-quote-header",
        [Cl.buffer(expiredQuote), Cl.uint(2)],
        address1
      );

      expect(result).toBeErr(Cl.uint(1003)); // ERR-QUOTE-EXPIRED
    });

    it("should handle buffer parse errors gracefully", () => {
      // Create a buffer that's too small to parse properly
      const malformedQuote = new Uint8Array(50); // Too small for parsing at offset 60
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "validate-quote-header",
        [Cl.buffer(malformedQuote), Cl.uint(2)],
        address1
      );
      
      expect(result).toBeErr(Cl.uint(1006)); // ERR-BUFFER-PARSE-ERROR
    });
  });

  describe("extract-uint16-be", () => {
    it("should correctly extract uint16 from buffer", () => {
      const buffer = new Uint8Array(1000);
      buffer[10] = 0x12; // High byte
      buffer[11] = 0x34; // Low byte
      // Expected: 0x1234 = 4660
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint16-be",
        [Cl.buffer(buffer), Cl.uint(10)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(4660));
    });

    it("should handle buffer parse error when offset is out of bounds", () => {
      const buffer = new Uint8Array(100);
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint16-be",
        [Cl.buffer(buffer), Cl.uint(100)], // Offset equals buffer length
        address1
      );
      
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should handle buffer parse error when insufficient bytes available", () => {
      const buffer = new Uint8Array(100);
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint16-be",
        [Cl.buffer(buffer), Cl.uint(99)], // Only 1 byte available, need 2
        address1
      );
      
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should extract from exact boundary (offset + size = buffer length)", () => {
      const buffer = new Uint8Array(10);
      buffer[8] = 0xAB;
      buffer[9] = 0xCD; // Last 2 bytes: 0xABCD = 43981
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint16-be",
        [Cl.buffer(buffer), Cl.uint(8)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(43981));
    });

    it("should handle zero values correctly", () => {
      const buffer = new Uint8Array(10); // All zeros by default
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint16-be",
        [Cl.buffer(buffer), Cl.uint(0)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(0));
    });

    it("should handle maximum uint16 value", () => {
      const buffer = new Uint8Array(10);
      buffer[5] = 0xFF;
      buffer[6] = 0xFF; // 0xFFFF = 65535
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint16-be",
        [Cl.buffer(buffer), Cl.uint(5)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(65535));
    });
  });

  describe("extract-bytes32", () => {
    it("should extract 32 bytes correctly", () => {
      const buffer = new Uint8Array(8192);
      // Set a test pattern in 32 bytes starting at offset 100
      for (let i = 0; i < 32; i++) {
        buffer[100 + i] = i + 1; // Values 1-32
      }
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-bytes32",
        [Cl.buffer(buffer), Cl.uint(100)],
        address1
      );
      
      expect(result).toBeOk(expect.objectContaining({ type: 'buffer' }));
    });

    it("should handle buffer parse error when offset is too close to end", () => {
      const buffer = new Uint8Array(8192);
      
      // Try to read 32 bytes starting at offset that leaves less than 32 bytes available
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-bytes32",
        [Cl.buffer(buffer), Cl.uint(8192 - 31)], // Only 31 bytes available, need 32
        address1
      );
      
      // The function returns a nested error (err (err u1006))
      expect(result).toBeErr(expect.anything());
      // Extract the inner error and check it's the correct error code  
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should fail when offset equals buffer length", () => {
      const buffer = new Uint8Array(100);
      
      // Try offset at exact buffer length - should fail on first byte access
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-bytes32",
        [Cl.buffer(buffer), Cl.uint(100)], // Offset at buffer length
        address1
      );
      
      // The function returns a nested error (err (err u1006))
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should fail when offset exceeds buffer length", () => {
      const buffer = new Uint8Array(100);
      
      // Try offset way beyond buffer
      const { result } = simnet.callReadOnlyFn(
        "executor", 
        "extract-bytes32",
        [Cl.buffer(buffer), Cl.uint(500)], // Far beyond buffer length
        address1
      );
      
      // The function returns a nested error (err (err u1006))
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should fail with empty buffer", () => {
      const buffer = new Uint8Array(0);
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-bytes32", 
        [Cl.buffer(buffer), Cl.uint(0)], // offset 0 should fail with empty buffer
        address1
      );
      
      // The function returns a nested error (err (err u1006))
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should work with minimum viable buffer (32 bytes)", () => {
      const buffer = new Uint8Array(32);
      buffer.fill(0xAA);
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-bytes32",
        [Cl.buffer(buffer), Cl.uint(0)], // Extract all 32 bytes
        address1
      );
      
      expect(result).toBeOk(expect.objectContaining({ type: 'buffer' }));
    });


    it("should verify extracted bytes content", () => {
      const buffer = new Uint8Array(64);
      for (let i = 0; i < 32; i++) {
        buffer[10 + i] = i + 1;
      }
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-bytes32", 
        [Cl.buffer(buffer), Cl.uint(10)],
        address1
      );
      
      expect(result).toBeOk(expect.objectContaining({ type: 'buffer' }));
      // Verify the buffer length is exactly 32 bytes (64 hex chars)
      const extractedBuffer = (result as any).value;
      expect(extractedBuffer.value.length).toBe(64);
    });

    it("should handle maximum offset (8192 - 32)", () => {
      const buffer = new Uint8Array(8192);
      buffer.fill(0xFF, 8160, 8192); // Fill last 32 bytes with 0xFF
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-bytes32",
        [Cl.buffer(buffer), Cl.uint(8160)], // Extract last 32 bytes
        address1
      );
      
      expect(result).toBeOk(expect.objectContaining({ type: 'buffer' }));
      
      // Verify the buffer content matches what we set (32 bytes of 0xFF = 64 'f' chars)
      const extractedBuffer = (result as any).value;
      expect(extractedBuffer.value.length).toBe(64); // 32 bytes = 64 hex chars
      expect(extractedBuffer.value).toBe('f'.repeat(64)); // All 0xFF
    });


    it("should handle very large offset gracefully", () => {
      const buffer = new Uint8Array(8192);
      
      // Try very large offset - should fail gracefully
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-bytes32",
        [Cl.buffer(buffer), Cl.uint(999999999999)], // Max uint32
        address1
      );
      
      // The function returns a nested error (err (err u1006))
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;  
      expect(innerError).toBeErr(Cl.uint(1006));
    });
  });

  describe("extract-uint64-be", () => {
    it("should correctly extract uint64 from buffer", () => {
      const buffer = new Uint8Array(1000);
      // Set up 0x0000000000000123 = 291 in decimal
      buffer[20] = 0x00;
      buffer[21] = 0x00;
      buffer[22] = 0x00;
      buffer[23] = 0x00;
      buffer[24] = 0x00;
      buffer[25] = 0x00;
      buffer[26] = 0x01;
      buffer[27] = 0x23;
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(20)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(291));
    });

    it("should handle zero value", () => {
      const buffer = new Uint8Array(1000);
      // All bytes are already 0x00
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(0)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(0));
    });

    it("should handle maximum uint64 value", () => {
      const buffer = new Uint8Array(1000);
      // Set all 8 bytes to 0xFF for max uint64: 18446744073709551615
      for (let i = 0; i < 8; i++) {
        buffer[10 + i] = 0xFF;
      }
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(10)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(18446744073709551615n));
    });

    it("should handle single byte set in different positions", () => {
      const buffer = new Uint8Array(1000);
      
      // Test most significant byte: 0x0100000000000000 = 72057594037927936
      buffer[50] = 0x01;
      const { result: result1 } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(50)],
        address1
      );
      expect(result1).toBeOk(Cl.uint(72057594037927936n));
      
      // Clear and test least significant byte: 0x0000000000000001 = 1
      buffer[50] = 0x00;
      buffer[57] = 0x01;
      const { result: result2 } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(50)],
        address1
      );
      expect(result2).toBeOk(Cl.uint(1));
    });

    it("should handle realistic timestamp value", () => {
      const buffer = new Uint8Array(1000);
      // Convert 1704067200 to hex bytes
      const timestamp = 1704067200;
      const timestampBig = BigInt(timestamp);
      
      // Convert to big-endian bytes
      for (let i = 0; i < 8; i++) {
        buffer[60 + i] = Number((timestampBig >> BigInt(8 * (7 - i))) & 0xFFn);
      }
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(60)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(timestamp));
    });

    it("should handle buffer parse error when offset equals buffer length", () => {
      const buffer = new Uint8Array(100);
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(100)], // Offset equals buffer length
        address1
      );
      
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should handle buffer parse error with insufficient bytes (7 bytes available)", () => {
      const buffer = new Uint8Array(10);
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(3)], // 10-3=7 bytes available, need 8
        address1
      );
      
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should handle empty buffer", () => {
      const buffer = new Uint8Array(0);
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be", 
        [Cl.buffer(buffer), Cl.uint(0)],
        address1
      );
      
      expect(result).toBeErr(expect.anything());
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });

    it("should handle minimum viable buffer (8 bytes)", () => {
      const buffer = new Uint8Array(8);
      buffer.fill(0x42); // Fill with 0x4242424242424242
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(0)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(0x4242424242424242n));
    });

    it("should handle large block height value", () => {
      const buffer = new Uint8Array(1000);
      // Block height 1000000 = 0x00000000000F4240
      buffer[100] = 0x00;
      buffer[101] = 0x00;
      buffer[102] = 0x00;
      buffer[103] = 0x00;
      buffer[104] = 0x00;
      buffer[105] = 0x0F;
      buffer[106] = 0x42;
      buffer[107] = 0x40;
      
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(100)],
        address1
      );
      
      expect(result).toBeOk(Cl.uint(1000000));
    });
    
    it("should handle buffer parse error when offset is too close to end", () => {
      const buffer = new Uint8Array(1000);
      
      // Try to read at offset 993 (need 8 bytes, but only 7 available)
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "extract-uint64-be",
        [Cl.buffer(buffer), Cl.uint(993)],
        address1
      );
      
      // The function returns a nested error (err (err u1006))
      expect(result).toBeErr(expect.anything());
      // Extract the inner error and check it's the correct error code
      const innerError = (result as any).value;
      expect(innerError).toBeErr(Cl.uint(1006));
    });
  });

  describe("get-executor-version", () => {
    it("should return correct version", () => {
      const { result } = simnet.callReadOnlyFn(
        "executor",
        "get-executor-version",
        [],
        address1
      );
      
      expect(result).toBeAscii("Executor-0.0.1");
    });
  });

  describe("get-our-chain", () => {
    it("should return correct chain ID", () => {
      const { result } = simnet.callReadOnlyFn(
        "executor", 
        "get-our-chain",
        [],
        address1
      );
      
      expect(result).toBeUint(1); 
    });
  });

  describe("request-execution", () => {
    // Helper to register a test payee and get their universal address
    function registerTestPayee(principal: string) {
      const { result } = simnet.callPublicFn(
        "addr32",
        "register",
        [Cl.principal(principal)],
        principal
      );
      expect(result).toBeOk(expect.anything());
      const okValue = (result as any).value;
      // Access the addr32 buffer from the tuple's value
      return okValue.value.addr32;
    }

    it("should execute successfully with valid quote and registered relayer", () => {
      // Register a relayer first
      const relayerAddr = accounts.get("wallet_2")!;
      const universalAddr = registerTestPayee(relayerAddr);
      
      // Convert Clarity buffer to Uint8Array for use in quote
      const hexString = (universalAddr as any).value;
      const payeeBytes = new Uint8Array([...Array(hexString.length / 2)].map((_, i) => 
        parseInt(hexString.substr(i * 2, 2), 16)
      ));
      
      // Create quote with the registered relayer's universal address as payee
      const validQuote = createSignedQuoteBuffer({
        srcChain: 1,      // OUR-CHAIN
        dstChain: 2,      // Target chain
        expiryTime: 9999999999, // Future timestamp placeholder
        payeeAddr: payeeBytes
      });

      const { result } = simnet.callPublicFn(
        "executor",
        "request-execution",
        [
          Cl.uint(2),                                    // dst-chain
          Cl.buffer(new Uint8Array(32).fill(0x12)),     // dst-addr  
          Cl.principal(address1),                        // refund-addr
          Cl.buffer(validQuote),                         // signed-quote-bytes
          Cl.buffer(new Uint8Array(100).fill(0x34)),    // request-bytes
          Cl.buffer(new Uint8Array(100).fill(0x56)),    // relay-instructions
          Cl.uint(1000000)                               // payment (1 STX)
        ],
        address1
      );

      expect(result).toBeOk(Cl.bool(true));
    });

    it("should fail with wrong source chain", () => {
      const relayerAddr = accounts.get("wallet_2")!;
      const universalAddr = registerTestPayee(relayerAddr);
      
      const invalidQuote = createSignedQuoteBuffer({
        srcChain: 99,     // Wrong source chain
        dstChain: 2,
        expiryTime: 9999999999, // Future timestamp placeholder
        payeeAddr: universalAddr
      });

      const { result } = simnet.callPublicFn(
        "executor",
        "request-execution", 
        [
          Cl.uint(2),
          Cl.buffer(new Uint8Array(32).fill(0x12)),
          Cl.principal(address1),
          Cl.buffer(invalidQuote),
          Cl.buffer(new Uint8Array(100).fill(0x34)),
          Cl.buffer(new Uint8Array(100).fill(0x56)),
          Cl.uint(1000000)
        ],
        address1
      );

      expect(result).toBeErr(Cl.uint(1001)); // ERR-QUOTE-SRC-CHAIN-MISMATCH
    });

    it("should fail with wrong destination chain", () => {
      const relayerAddr = accounts.get("wallet_2")!;
      const universalAddr = registerTestPayee(relayerAddr);
      
      const invalidQuote = createSignedQuoteBuffer({
        srcChain: 1,
        dstChain: 99,     // Wrong destination chain
        expiryTime: 9999999999, // Future timestamp placeholder
        payeeAddr: universalAddr
      });

      const { result } = simnet.callPublicFn(
        "executor",
        "request-execution",
        [
          Cl.uint(2),     // Expecting chain 2, but quote says 99
          Cl.buffer(new Uint8Array(32).fill(0x12)),
          Cl.principal(address1),
          Cl.buffer(invalidQuote),
          Cl.buffer(new Uint8Array(100).fill(0x34)),
          Cl.buffer(new Uint8Array(100).fill(0x56)),
          Cl.uint(1000000)
        ],
        address1
      );

      expect(result).toBeErr(Cl.uint(1002)); // ERR-QUOTE-DST-CHAIN-MISMATCH
    });

    it("should fail with expired quote", () => {
      const relayerAddr = accounts.get("wallet_2")!;
      const universalAddr = registerTestPayee(relayerAddr);
      
      const expiredQuote = createSignedQuoteBuffer({
        srcChain: 1,
        dstChain: 2,
        expiryTime: 1000000000, // Past timestamp placeholder // 1 hour ago (expired)
        payeeAddr: universalAddr
      });

      const { result } = simnet.callPublicFn(
        "executor",
        "request-execution",
        [
          Cl.uint(2),
          Cl.buffer(new Uint8Array(32).fill(0x12)),
          Cl.principal(address1),
          Cl.buffer(expiredQuote),
          Cl.buffer(new Uint8Array(100).fill(0x34)),
          Cl.buffer(new Uint8Array(100).fill(0x56)),
          Cl.uint(1000000)
        ],
        address1
      );

      expect(result).toBeErr(Cl.uint(1003)); // ERR-QUOTE-EXPIRED
    });

    it("should fail with unregistered relayer", () => {
      // Create quote with unregistered payee address
      const unregisteredPayee = new Uint8Array(32);
      unregisteredPayee.fill(0xFF); // Address that's not registered
      
      const validQuote = createSignedQuoteBuffer({
        srcChain: 1,
        dstChain: 2,
        expiryTime: 9999999999, // Future timestamp placeholder
        payeeAddr: unregisteredPayee
      });

      const { result } = simnet.callPublicFn(
        "executor",
        "request-execution",
        [
          Cl.uint(2),
          Cl.buffer(new Uint8Array(32).fill(0x12)),
          Cl.principal(address1),
          Cl.buffer(validQuote),
          Cl.buffer(new Uint8Array(100).fill(0x34)),
          Cl.buffer(new Uint8Array(100).fill(0x56)),
          Cl.uint(1000000)
        ],
        address1
      );

      expect(result).toBeErr(Cl.uint(1004)); // ERR-UNREGISTERED-RELAYER
    });

    it("should emit correct event data", () => {
      const relayerAddr = accounts.get("wallet_3")!;
      const universalAddr = registerTestPayee(relayerAddr);
      
      // Convert Clarity buffer to Uint8Array for use in quote
      const hexString = (universalAddr as any).value;
      const payeeBytes = new Uint8Array([...Array(hexString.length / 2)].map((_, i) => 
        parseInt(hexString.substr(i * 2, 2), 16)
      ));
      
      const validQuote = createSignedQuoteBuffer({
        srcChain: 1,
        dstChain: 3,
        expiryTime: 9999999999, // Future timestamp placeholder
        payeeAddr: payeeBytes
      });

      const dstAddr = new Uint8Array(32).fill(0x78);
      const requestBytes = new Uint8Array(100).fill(0x9A);
      const relayInstructions = new Uint8Array(100).fill(0xBC);

      const { result, events } = simnet.callPublicFn(
        "executor",
        "request-execution",
        [
          Cl.uint(3),                          // dst-chain
          Cl.buffer(dstAddr),                  // dst-addr
          Cl.principal(address1),              // refund-addr
          Cl.buffer(validQuote),               // signed-quote-bytes
          Cl.buffer(requestBytes),             // request-bytes
          Cl.buffer(relayInstructions),        // relay-instructions
          Cl.uint(2000000)                     // payment (2 STX)
        ],
        address1
      );

      // Verify the transaction succeeded
      expect(result).toBeOk(Cl.bool(true));

      // Should have 2 events: STX transfer + print event
      expect(events.length).toBe(2);
      
      // Find the STX transfer event
      const transferEvent = events.find(e => e.event === "stx_transfer_event");
      expect(transferEvent).toBeTruthy();
      expect((transferEvent as any).data.amount).toBe("2000000");
      
      // Find the print event (our custom event)
      const printEvent = events.find(e => e.event === "print_event");
      expect(printEvent).toBeTruthy();
      
      if (printEvent) {
        // The print event data should contain our event structure
        const eventData = (printEvent as any).data.value.value;
        
        // Verify all the key fields match what we sent
        expect(eventData.event.value).toBe("RequestForExecution");
        expect(eventData["dst-chain"].value).toBe(3n);
        expect(eventData["amount-paid"].value).toBe(2000000n);
        expect(eventData["refund-addr"].value).toBe(address1);
        
        // Verify buffer fields have correct values
        expect(eventData["dst-addr"].value).toBe("78".repeat(32)); // All 0x78
        expect(eventData["request-bytes"].value).toBe("9a".repeat(100)); // All 0x9A
        expect(eventData["relay-instructions"].value).toBe("bc".repeat(100)); // All 0xBC
        expect(eventData["quoter-address"].value).toBe("112233445566778899aabbccddeeff0011223344"); // Default quoter
        
        // Verify signed-quote buffer structure and content
        expect(eventData["signed-quote"]).toBeDefined();
        expect(eventData["signed-quote"].type).toBe("buffer");
        
        const signedQuoteHex = eventData["signed-quote"].value;
        expect(signedQuoteHex).toBeDefined();
        expect(signedQuoteHex.length).toBeGreaterThan(0);
        
        // Verify the signed quote contains the quoter address we set (at offset 4)
        // Format: [4 bytes version/padding] + [20 bytes quoter] + [32 bytes payee] + [quote data...]
        const expectedQuoterHex = "112233445566778899aabbccddeeff0011223344";
        const quoterInQuote = signedQuoteHex.substr(8, 40); // Skip 4 bytes (8 hex chars), take 20 bytes (40 hex chars)
        expect(quoterInQuote).toBe(expectedQuoterHex);
        
        // Verify the payee address is present at offset 24 (after 4 bytes version + 20 bytes quoter)
        const expectedPayeeHex = (universalAddr as any).value;
        const payeeInQuote = signedQuoteHex.substr(48, 64); // Skip 24 bytes (48 hex chars), take 32 bytes (64 hex chars)
        expect(payeeInQuote).toBe(expectedPayeeHex);
        
        // Verify source chain (uint16 at offset 56: 4 + 20 + 32 bytes)
        const srcChainHex = signedQuoteHex.substr(112, 4); // 2 bytes = 4 hex chars
        expect(srcChainHex).toBe("0001"); // Source chain = 1
        
        // Verify destination chain (uint16 at offset 58: 4 + 20 + 32 + 2 bytes) 
        const dstChainHex = signedQuoteHex.substr(116, 4); // 2 bytes = 4 hex chars
        expect(dstChainHex).toBe("0003"); // Destination chain = 3
      }
    });
  });
});

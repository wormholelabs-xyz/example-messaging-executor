
import { describe, expect, it } from "vitest";
import { Cl } from "@stacks/transactions";

const accounts = simnet.getAccounts();
const address1 = accounts.get("wallet_1")!;
const address2 = accounts.get("wallet_2")!;
const address3 = accounts.get("wallet_3")!;

describe("Executor State Contract Tests", () => {
  it("ensures simnet is well initialised", () => {
    expect(simnet.blockHeight).toBeDefined();
  });

  describe("register-relayer", () => {
    it("should register a new relayer successfully", () => {
      const { result } = simnet.callPublicFn(
        "executor-state",
        "register-relayer", 
        [Cl.principal(address1)],
        address1
      );
      
      // Should return a buffer (universal address)
      expect(result).toBeOk(expect.objectContaining({ type: 'buffer' }));
      
      const universalAddr = (result as any).value;
      
      // Should be able to look up the registered address
      const { result: lookupResult } = simnet.callReadOnlyFn(
        "executor-state",
        "universal-addr-to-principal",
        [universalAddr],
        address1
      );
      
      // Lookup should return the original principal
      expect(lookupResult).toBeSome(Cl.principal(address1));
      
      // Should also work with the map getter
      const { result: mapResult } = simnet.callReadOnlyFn(
        "executor-state",
        "relayer-to-stacks-get",
        [universalAddr],
        address1
      );
      
      expect(mapResult).toBeSome(Cl.principal(address1));
    });

    it("should prevent duplicate relayer registration", () => {
      // First registration should succeed
      const { result: result1 } = simnet.callPublicFn(
        "executor-state",
        "register-relayer",
        [Cl.principal(address2)],
        address2
      );
      expect(result1).toBeOk(expect.objectContaining({ type: 'buffer' }));
      
      const universalAddr1 = (result1 as any).value;

      // Verify first registration worked
      const { result: lookupResult1 } = simnet.callReadOnlyFn(
        "executor-state",
        "universal-addr-to-principal",
        [universalAddr1],
        address1
      );
      expect(lookupResult1).toBeSome(Cl.principal(address2));

      // Second registration should fail with specific error
      const { result: result2 } = simnet.callPublicFn(
        "executor-state", 
        "register-relayer",
        [Cl.principal(address2)],
        address2
      );
      expect(result2).toBeErr(Cl.uint(20001)); // ERR_STATE_RELAYER_EXISTS
      
      // Original mapping should still work after failed duplicate attempt
      const { result: lookupResult2 } = simnet.callReadOnlyFn(
        "executor-state",
        "universal-addr-to-principal",
        [universalAddr1],
        address1
      );
      expect(lookupResult2).toBeSome(Cl.principal(address2));
    });

    it("should allow different addresses to register", () => {
      const { result: result1 } = simnet.callPublicFn(
        "executor-state",
        "register-relayer",
        [Cl.principal(address1)],
        address1
      );
      expect(result1).toBeOk(expect.objectContaining({ type: 'buffer' }));

      const { result: result2 } = simnet.callPublicFn(
        "executor-state",
        "register-relayer", 
        [Cl.principal(address2)],
        address2
      );
      expect(result2).toBeOk(expect.objectContaining({ type: 'buffer' }));

      const universalAddr1 = (result1 as any).value;
      const universalAddr2 = (result2 as any).value;

      // Should return different universal addresses
      expect(universalAddr1).not.toEqual(universalAddr2);
      
      // Both should be queryable and return correct principals
      const { result: lookup1 } = simnet.callReadOnlyFn(
        "executor-state",
        "universal-addr-to-principal",
        [universalAddr1],
        address3
      );
      expect(lookup1).toBeSome(Cl.principal(address1));
      
      const { result: lookup2 } = simnet.callReadOnlyFn(
        "executor-state",
        "universal-addr-to-principal",
        [universalAddr2],
        address3
      );
      expect(lookup2).toBeSome(Cl.principal(address2));
      
      // Cross-lookup should not work (addr1 should not find addr2's principal)
      const { result: crossLookup1 } = simnet.callReadOnlyFn(
        "executor-state",
        "universal-addr-to-principal",
        [universalAddr1],
        address3
      );
      expect(crossLookup1).not.toEqual(Cl.some(Cl.principal(address2)));
    });

    it("should return consistent universal address for same principal", () => {
      // Register relayer
      const { result: registerResult } = simnet.callPublicFn(
        "executor-state",
        "register-relayer",
        [Cl.principal(address3)], 
        address3
      );
      expect(registerResult).toBeOk(expect.objectContaining({ type: 'buffer' }));
      
      const universalAddr = (registerResult as any).value;

      // Lookup should return the same principal
      const { result: lookupResult } = simnet.callReadOnlyFn(
        "executor-state",
        "universal-addr-to-principal",
        [universalAddr],
        address1
      );
      expect(lookupResult).toBeSome(Cl.principal(address3));
    });
  });

  describe("universal-addr-to-principal", () => {
    it("should return registered principal for valid universal address", () => {
      // Register relayer first
      const { result: registerResult } = simnet.callPublicFn(
        "executor-state",
        "register-relayer",
        [Cl.principal(address1)],
        address1
      );
      expect(registerResult).toBeOk(expect.objectContaining({ type: 'buffer' }));
      
      const universalAddr = (registerResult as any).value;

      // Lookup should work from any caller
      const { result: lookupResult1 } = simnet.callReadOnlyFn(
        "executor-state", 
        "universal-addr-to-principal",
        [universalAddr],
        address2
      );
      expect(lookupResult1).toBeSome(Cl.principal(address1));
      
      // Should also work from different caller
      const { result: lookupResult2 } = simnet.callReadOnlyFn(
        "executor-state", 
        "universal-addr-to-principal",
        [universalAddr],
        address3
      );
      expect(lookupResult2).toBeSome(Cl.principal(address1));
      
      // Both lookups should return identical results
      expect(lookupResult1).toEqual(lookupResult2);
    });

    it("should return none for unregistered universal address", () => {
      // Create a fake universal address (32 bytes of zeros)
      const fakeUniversalAddr = new Uint8Array(32);
      
      const { result } = simnet.callReadOnlyFn(
        "executor-state",
        "universal-addr-to-principal", 
        [Cl.buffer(fakeUniversalAddr)],
        address1
      );
      expect(result).toBeNone();
    });
  });

  describe("relayer-to-stacks-get", () => {
    it("should return registered principal for valid universal address", () => {
      // Register relayer
      const { result: registerResult } = simnet.callPublicFn(
        "executor-state",
        "register-relayer",
        [Cl.principal(address2)],
        address2
      );
      expect(registerResult).toBeOk(expect.objectContaining({ type: 'buffer' }));
      
      const universalAddr = (registerResult as any).value;

      // Test the map getter directly
      const { result: mapResult } = simnet.callReadOnlyFn(
        "executor-state",
        "relayer-to-stacks-get",
        [universalAddr],
        address1
      );
      expect(mapResult).toBeSome(Cl.principal(address2));
    });

    it("should return none for unregistered address", () => {
      const fakeUniversalAddr = new Uint8Array(32);
      
      const { result } = simnet.callReadOnlyFn(
        "executor-state",
        "relayer-to-stacks-get",
        [Cl.buffer(fakeUniversalAddr)],
        address1
      );
      expect(result).toBeNone();
    });
  });
});

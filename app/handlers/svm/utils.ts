import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {
  CompiledInstruction,
  Connection,
  Message,
  MessageCompiledInstruction,
  MessageV0,
  PublicKey,
  VersionedMessage,
  VersionedTransactionResponse,
} from "@solana/web3.js";

// borrowed from https://github.com/wormhole-foundation/wormhole-dashboard/blob/7ca085ed94a2573bcb2247e7e2d536c4989e47f1/watcher/src/utils/solana.ts
export const isLegacyMessage = (
  message: Message | MessageV0,
): message is Message => {
  return message.version === "legacy";
};
export const normalizeCompileInstruction = (
  instruction: CompiledInstruction | MessageCompiledInstruction,
): MessageCompiledInstruction => {
  if ("accounts" in instruction) {
    return {
      accountKeyIndexes: instruction.accounts,
      data: bs58.decode(instruction.data),
      programIdIndex: instruction.programIdIndex,
    };
  } else {
    return instruction;
  }
};
export async function getAllKeys(
  connection: Connection,
  res: VersionedTransactionResponse,
): Promise<PublicKey[]> {
  const message: VersionedMessage = res.transaction.message;
  let accountKeys = isLegacyMessage(message)
    ? message.accountKeys
    : message.staticAccountKeys;

  // If the message contains an address table lookup, we need to resolve the addresses
  // before looking for the programIdIndex
  if (message.addressTableLookups.length > 0) {
    const lookupPromises = message.addressTableLookups.map(async (atl) => {
      const lookupTableAccount = await connection
        .getAddressLookupTable(atl.accountKey)
        .then((res) => res.value);

      if (!lookupTableAccount)
        throw new Error("lookupTableAccount is null, cant resolve addresses");

      // Important to return the addresses in the order they're specified in the
      // address table lookup object. Note writable comes first, then readable.
      return [
        atl.accountKey,
        atl.writableIndexes.map((i) => lookupTableAccount.state.addresses[i]),
        atl.readonlyIndexes.map((i) => lookupTableAccount.state.addresses[i]),
      ] as [PublicKey, PublicKey[], PublicKey[]];
    });

    // Lookup all addresses in parallel
    const lookups = await Promise.all(lookupPromises);

    // Ensure the order is maintained for lookups
    // Static, Writable, Readable
    // ref: https://github.com/gagliardetto/solana-go/blob/main/message.go#L414-L464
    const writable: PublicKey[] = [];
    const readable: PublicKey[] = [];
    for (const atl of message.addressTableLookups) {
      const table = lookups.find((l) => l[0].equals(atl.accountKey));
      if (!table) throw new Error("Could not find address table lookup");
      writable.push(...table[1]);
      readable.push(...table[2]);
    }

    accountKeys.push(...writable.concat(readable));
  }
  return accountKeys;
}

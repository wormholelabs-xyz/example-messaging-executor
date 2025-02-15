import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import {
  Connection,
  Message,
  MessageV0,
  PublicKey,
  VersionedMessage,
  VersionedTransactionResponse,
} from "@solana/web3.js";
import "@wormhole-foundation/sdk-definitions-ntt"; // register definition for parsing
import { deserializePostMessage } from "@wormhole-foundation/sdk-solana-core";
import { fromBytes, fromHex, padHex, sha256 } from "viem";
import { NttHandler } from "..";

// borrowed from https://github.com/wormhole-foundation/wormhole-dashboard/blob/7ca085ed94a2573bcb2247e7e2d536c4989e47f1/watcher/src/utils/solana.ts
export const isLegacyMessage = (
  message: Message | MessageV0,
): message is Message => {
  return message.version === "legacy";
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

export const svmNttHandler: NttHandler = {
  async getTransceivers(chainInfo, address, blockNumber) {
    const connection = new Connection(chainInfo.rpc);
    const programId = new PublicKey(fromHex(address, "bytes"));
    // spec https://github.com/wormhole-foundation/native-token-transfers/blob/3311787ab22087f5c10ab08edb6a2a5e3f7afd77/solana/programs/example-native-token-transfers/src/registered_transceiver.rs#L3-L10
    const registeredTransceiverDiscriminator = fromHex(
      sha256(Buffer.from("account:RegisteredTransceiver")),
      "bytes",
    ).subarray(0, 8);
    const registeredTransceiverAccounts = await connection.getProgramAccounts(
      programId,
      {
        filters: [
          { dataSize: 42 },
          {
            memcmp: {
              offset: 0,
              bytes: bs58.encode(registeredTransceiverDiscriminator),
            },
          },
        ],
      },
    );
    const transceiverPubkeys = registeredTransceiverAccounts.map(
      (t) => new PublicKey(t.account.data.subarray(10, 42)),
    );
    const transceivers = [];
    for (const pubkey of transceiverPubkeys) {
      // TODO: try calling transceiverType
      // https://github.com/wormhole-foundation/native-token-transfers/blob/3311787ab22087f5c10ab08edb6a2a5e3f7afd77/solana/ts/sdk/ntt.ts#L142-L143
      if (pubkey.equals(programId)) {
        // a self-referencing NTT program is assumed to be wormhole
        transceivers.push({
          address: fromBytes(pubkey.toBytes(), "hex"),
          type: "wormhole",
        });
      }
    }
    return transceivers;
  },
  async getTransferMessages(chainInfo, hash, address, messageId) {
    const connection = new Connection(chainInfo.rpc);
    // on SVM, the outbox account *is* the message ID
    const outboxAccount = new PublicKey(fromHex(messageId, "bytes"));
    // it is possible for the transceiver sends to be spread across multiple transactions
    // this could be found using `getSignaturesForAddress`, but for now assume they all fit into the same transaction
    const tx = await connection.getTransaction(
      bs58.encode(fromHex(hash, "bytes")),
      {
        maxSupportedTransactionVersion: 0,
      },
    );
    if (!tx) {
      throw new Error(`failed to fetch tx ${tx}`);
    }
    const accounts = await getAllKeys(connection, tx);
    // search for instructions that match a supported transceiver type
    const releaseWormholeOutboundDiscriminator = "0xca5733ad8ea0bccc";
    const outboxIdx = accounts.findIndex((a) => a.equals(outboxAccount));
    if (outboxIdx === -1) {
      throw new Error(`failed to find account ${outboxIdx}`);
    }
    const supportedMessages = [];
    // currently only support top-level instructions
    for (
      let outerIdx = 0;
      outerIdx < tx.transaction.message.compiledInstructions.length;
      outerIdx++
    ) {
      const outerIx = tx.transaction.message.compiledInstructions[outerIdx];
      if (outerIx.accountKeyIndexes.includes(outboxIdx)) {
        // this instruction involves the target outbox account
        try {
          if (
            tx.meta?.innerInstructions &&
            fromBytes(outerIx.data.subarray(0, 8), "hex") ===
              releaseWormholeOutboundDiscriminator
          ) {
            // this is a release_wormhole_outbound instruction
            // search for a wormhole message
            for (const innerIxs of tx.meta.innerInstructions) {
              if (innerIxs.index === outerIdx) {
                for (const innerIx of innerIxs.instructions) {
                  const data = bs58.decode(innerIx.data);
                  // TODO: support shim emissions
                  if (data[0] === 0x01) {
                    // first byte matches the ix for a core bridge postMessage instruction
                    // the message account index is at the first index
                    const accountId = accounts[innerIx.accounts[1]];
                    const acctInfo = await connection.getAccountInfo(accountId);
                    if (!acctInfo?.data) {
                      throw new Error("No data found in message account");
                    }
                    const { emitterAddress, sequence } = deserializePostMessage(
                      acctInfo.data,
                    );
                    const emitterHex = fromBytes(emitterAddress.address, "hex");
                    supportedMessages.push({
                      address: emitterHex,
                      type: "wormhole",
                      id: `${chainInfo.chainId}/${padHex(emitterHex, { dir: "left", size: 32 }).substring(2)}/${sequence.toString()}`,
                    });
                  }
                }
              }
            }
          }
        } catch (e) {}
      }
    }
    return supportedMessages;
  },
};

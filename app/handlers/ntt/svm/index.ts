import { AnchorProvider, BN, Program, Wallet, web3 } from "@coral-xyz/anchor";
import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { Connection, Keypair, PublicKey } from "@solana/web3.js";
import { deserialize, Wormhole } from "@wormhole-foundation/sdk-connect";
import "@wormhole-foundation/sdk-definitions-ntt"; // register definition for parsing
import { SolanaAddress, SolanaPlatform } from "@wormhole-foundation/sdk-solana";
import { deserializePostMessage } from "@wormhole-foundation/sdk-solana-core";
import "@wormhole-foundation/sdk-solana-ntt"; // register solana
import { fromBytes, fromHex, padHex } from "viem";
import { NttHandler } from "..";
import { getAllKeys } from "../../svm/utils";
import { Manager } from "./manager";
import ManagerIdl from "./manager.json";

function checkBit(bn: BN, bitIndex: number) {
  const mask = new BN(1).shln(bitIndex);
  return bn.and(mask).gtn(0);
}

export const svmNttHandler: NttHandler = {
  async getEnabledTransceivers(chainInfo, address, blockNumber) {
    const connection = new Connection(chainInfo.rpc);
    const programId = new PublicKey(fromHex(address, "bytes"));
    // create provider with dummy wallet
    const provider = new AnchorProvider(
      connection,
      new Wallet(Keypair.generate()),
    );
    const overrideIdl = {
      ...ManagerIdl,
      address: programId.toString(),
    };
    const program = new Program<Manager>(overrideIdl as Manager, provider);
    const config = await program.account.config.fetch(
      PublicKey.findProgramAddressSync([Buffer.from("config")], programId)[0],
    );
    const registeredTransceiverAccounts =
      await program.account.registeredTransceiver.all();
    const enabledTransceiverPubkeys = registeredTransceiverAccounts
      .filter((t) => checkBit(config.enabledTransceivers.map, t.account.id))
      .map((t) => t.account.transceiverAddress);
    const enabledTransceivers = [];
    for (const pubkey of enabledTransceiverPubkeys) {
      // TODO: try calling transceiverType
      // https://github.com/wormhole-foundation/native-token-transfers/blob/3311787ab22087f5c10ab08edb6a2a5e3f7afd77/solana/ts/sdk/ntt.ts#L142-L143
      if (pubkey.equals(programId)) {
        // a self-referencing NTT program is assumed to be wormhole
        enabledTransceivers.push({
          address: fromBytes(pubkey.toBytes(), "hex"),
          type: "wormhole",
        });
      }
    }
    return enabledTransceivers;
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

  async relayNTTv1(c, r, n, t) {
    if (!c.privateKey) {
      throw new Error(`No private key configured`);
    }
    const transceivers = await this.getEnabledTransceivers(c, r.dstAddr);

    // TODO: fund, use, and destroy ephemeral keypair for relay
    const sigs: string[] = [];
    const connection = new web3.Connection(c.rpc, "confirmed");
    const programId = new PublicKey(fromHex(r.dstAddr, "bytes"));
    const payer = web3.Keypair.fromSecretKey(fromHex(c.privateKey, "bytes"));
    const provider = new AnchorProvider(connection, new Wallet(payer));

    // get mint from config
    // TODO: avoid fetching config twice (also fetched by getEnabledTransceivers)
    const overrideIdl = {
      ...ManagerIdl,
      address: programId.toString(),
    };
    const program = new Program<Manager>(overrideIdl as Manager, provider);
    const config = await program.account.config.fetch(
      PublicKey.findProgramAddressSync([Buffer.from("config")], programId)[0],
    );

    const wh = new Wormhole("Testnet", [SolanaPlatform]);
    const s = wh.getChain("Solana");
    const contracts = {
      ntt: {
        chain: "Solana", // TODO: chain id to chain?
        manager: programId.toString(),
        token: config.mint.toString(),
        transceiver: transceivers.reduce(
          (obj, t) => ({
            ...obj,
            [t.type]: new PublicKey(fromHex(t.address, "bytes")).toString(),
          }),
          {},
        ),
      },
    };
    const ntt = await s.getProtocol("Ntt", contracts);
    const txs = ntt.redeem(
      // TODO: this code only implicitly handles wormhole types, but the underlying type only supports wormhole types
      t.map((p) =>
        deserialize("Ntt:WormholeTransfer", Buffer.from(p.payload, "base64")),
      ),
      new SolanaAddress(payer.publicKey),
    );
    for await (const tx of txs) {
      sigs.push(
        await provider.sendAndConfirm(
          tx.transaction.transaction,
          tx.transaction.signers,
          { commitment: "confirmed" },
        ),
      );
    }
    return sigs;
  },
};

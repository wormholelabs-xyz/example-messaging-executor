import { AnchorProvider, Program, Wallet, web3 } from "@coral-xyz/anchor";
import { deserialize } from "@wormhole-foundation/sdk-definitions";
import { SolanaWormholeCore } from "@wormhole-foundation/sdk-solana-core";
import axios from "axios";
import bs58 from "bs58";
import { Handler } from ".";
import { BinaryReader } from "../BinaryReader";
import {
  ModularMessageRequest,
  RequestForExecution,
  VAAv1Request,
} from "../requestForExecution";
import { SignedQuote } from "../signedQuote";
import { ChainInfo } from "../types";
import { Relayer } from "./svm/relayer";
import RelayerIdl from "./svm/relayer.json";

function parseInstructionData(data: string): RequestForExecution | null {
  const reader = new BinaryReader(bs58.decode(data));
  const discriminator = reader.readHex(8);
  if (discriminator === "0x6d6b572597c07773") {
    // amount: u64,
    const amtPaid = reader.readUint64LE();
    // dst_chain: u16,
    const dstChain = reader.readUint16LE();
    // _dst_addr: [u8; 32],
    const dstAddr = reader.readHex(32);
    // _refund_addr: Pubkey,
    const refundAddr = reader.readHex(32);
    // signed_quote_bytes: Vec<u8>,
    const signedQuoteLen = reader.readUint32LE();
    const signedQuoteBytes = reader.readHex(signedQuoteLen);
    const quoterAddress = SignedQuote.from(signedQuoteBytes).quoterAddress;
    // _request_bytes: Vec<u8>,
    const requestLen = reader.readUint32LE();
    const requestBytes = reader.readHex(requestLen);
    // _relay_instructions: Vec<u8>,
    const relayInstructionsLen = reader.readUint32LE();
    const relayInstructionsBytes = reader.readHex(relayInstructionsLen);
    return {
      amtPaid,
      dstAddr,
      dstChain,
      quoterAddress,
      refundAddr,
      relayInstructionsBytes,
      requestBytes,
      signedQuoteBytes,
    };
  }
  return null;
}

export const svmHandler: Handler = {
  getGasPrice: async (rpc: string) => {
    // TODO: get priority fee
    return BigInt("1000000");
  },
  getRequest: async (
    rpc: string,
    executorAddress: string,
    id: BinaryReader,
  ) => {
    const transactionHash = bs58.encode(id.readUint8Array(64));
    // TODO: fetch with sdk / handle versioned transactions
    const response = await axios.post(rpc, {
      jsonrpc: "2.0",
      id: 1,
      method: "getTransaction",
      params: [transactionHash, "json"],
    });
    if ("Ok" in response.data?.result?.meta?.status) {
      // TODO: this code picks up the first executor instruction in a transaction
      // TODO: use account lookup code from watcher
      const accountKeys =
        response.data.result.transaction?.message?.accountKeys;
      const executorIndex = accountKeys.indexOf(executorAddress);
      if (executorIndex >= 0) {
        const topIxs = response.data.result.transaction.message.instructions;
        for (const ix of topIxs) {
          if (ix.programIdIndex === executorIndex) {
            return parseInstructionData(ix.data);
          }
        }
        const innerIxs = response.data.result.meta.innerInstructions;
        for (const inner of innerIxs) {
          for (const ix of inner.instructions) {
            if (ix.programIdIndex === executorIndex) {
              return parseInstructionData(ix.data);
            }
          }
        }
      }
    }
    return null;
  },
  relayVAAv1: async (
    c: ChainInfo,
    r: RequestForExecution,
    v: VAAv1Request,
    b: string,
  ) => {
    if (!c.privateKey) {
      throw new Error(`No private key configured`);
    }
    const vaa = Buffer.from(b, "base64");
    const sigStart = 6;
    const numSigners = vaa[5];
    const sigLength = 66;
    const vaaBody = vaa.subarray(sigStart + sigLength * numSigners);
    const connection = new web3.Connection(c.rpc, "confirmed");
    const payer = web3.Keypair.fromSecretKey(
      new Uint8Array(Buffer.from(c.privateKey.substring(2), "hex")),
    );
    const provider = new AnchorProvider(connection, new Wallet(payer));
    const overrideIdl = {
      ...RelayerIdl,
      address: new web3.PublicKey(
        Buffer.from(r.dstAddr.substring(2), "hex"),
      ).toString(),
    };
    const program = new Program<Relayer>(overrideIdl as Relayer, provider);
    const result = await program.methods.executeVaaV1(vaaBody).view();
    const PAYER = new web3.PublicKey(
      Buffer.from("payer000000000000000000000000000"),
    ).toString();
    const payerIdx = result.accounts.findIndex(
      (a: any) => a.pubkey.toString() === PAYER,
    );
    if (payerIdx === -1) {
      throw new Error("Cannot find payer placeholder");
    }
    // TODO: fund, use, and destroy ephemeral keypair for relay
    result.accounts[payerIdx].pubkey = payer.publicKey;
    const sigs: string[] = [];
    // TODO: check for derived VAA address and only post if it exists
    const VAA = deserialize("Uint8Array", Buffer.from(b, "base64"));
    // TODO: don't hardcode core bridge address
    const wh = new SolanaWormholeCore("Testnet", "Solana", connection, {
      coreBridge: "3u8hJUVTA4jH1wYAyUur7FFZVQ8H635K3tSHHF4ssjQ5",
    });
    const txs = wh.verifyMessage(payer.publicKey, VAA);
    for await (const tx of txs) {
      sigs.push(
        await provider.sendAndConfirm(
          tx.transaction.transaction,
          tx.transaction.signers,
          { commitment: "confirmed" },
        ),
      );
    }
    const tx = new web3.Transaction();
    const ix = new web3.TransactionInstruction({
      programId: result.programId,
      keys: result.accounts,
      data: result.data,
    });
    tx.add(ix);
    sigs.push(
      await provider.sendAndConfirm(tx, [], { commitment: "confirmed" }),
    );
    return sigs;
  },
  relayMM: async (
    c: ChainInfo,
    r: RequestForExecution,
    m: ModularMessageRequest,
  ) => {
    if (!c.privateKey) {
      throw new Error(`No private key configured`);
    }
    throw new Error("Unsupported");
  },
};

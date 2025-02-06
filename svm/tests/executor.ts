import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Executor } from "../target/types/executor";
import { BN } from "bn.js";
import { BinaryWriter } from "./BinaryWriter";
import { expect, use } from "chai";
import chaiAsPromised from "chai-as-promised";

use(chaiAsPromised);

describe("executor", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.Executor as Program<Executor>;

  const encodeSignedQuoteHeader = (
    prefix: string,
    quoterAddress: string,
    payeeAddress: anchor.web3.PublicKey,
    srcChain: number,
    dstChain: number,
    expiryTime: bigint,
    additionalBytes?: string, // just for testing
  ) =>
    new BinaryWriter(68)
      .writeUint8Array(new Uint8Array(Buffer.from(prefix, "ascii")))
      .writeHex(quoterAddress)
      .writeUint8Array(payeeAddress.toBytes())
      .writeUint16(srcChain)
      .writeUint16(dstChain)
      .writeUint64(expiryTime)
      .writeHex(additionalBytes || "00") // just for testing
      .data();

  it("Requests execution!", async () => {
    await expect(
      program.methods
        .requestForExecution({
          amount: new BN(1),
          dstChain: 2,
          dstAddr: [
            ...Buffer.from(
              "0000000000000000000000000000000000000000000000000000000000000000",
              "hex",
            ),
          ],
          refundAddr: program.provider.publicKey!,
          signedQuoteBytes: Buffer.from(
            encodeSignedQuoteHeader(
              "EQ01",
              "0x0000000000000000000000000000000000000000",
              program.provider.publicKey!,
              1,
              2,
              BigInt(Date.now() + 1_000_000) / BigInt(1000),
            ),
          ),
          requestBytes: Buffer.from("", "hex"),
          relayInstructions: Buffer.from("", "hex"),
        })
        .accounts({
          payee: program.provider.publicKey!,
        })
        .rpc(),
    ).to.be.fulfilled;
  });

  it("Requests execution with real quote, v1 VAA request, and relay instruction!", async () => {
    await expect(
      program.methods
        .requestForExecution({
          amount: new BN(1),
          dstChain: 2,
          dstAddr: [
            ...Buffer.from(
              "0000000000000000000000000000000000000000000000000000000000000000",
              "hex",
            ),
          ],
          refundAddr: program.provider.publicKey!,
          signedQuoteBytes: Buffer.from(
            encodeSignedQuoteHeader(
              "EQ01",
              "0x0000000000000000000000000000000000000000",
              program.provider.publicKey!,
              1,
              2,
              BigInt(Date.now() + 1_000_000) / BigInt(1000),
              "0000000000002710000000003b9aca0700001d624add080000000625b3cb4600b69fffad8549dd87b875a85f6341283bc3cc61758e5b929cfa6913c8727af7a2776c66584cc260cb05d505c852187c0a29a7b774e1d05815020f774013b30fe11c",
            ),
          ),
          requestBytes: Buffer.from(
            "4552563100020000000000000000000000009ee7a4e1dc11226d90db33f326168cf33b2456cc0000000000000000",
            "hex",
          ),
          relayInstructions: Buffer.from(
            "01000000000000000000000000000f424000000000000000000000000000000000",
            "hex",
          ),
        })
        .accounts({
          payee: program.provider.publicKey!,
        })
        .rpc(),
    ).to.be.fulfilled;
  });

  it("Requests execution with real quote, MM request, and relay instruction!", async () => {
    await expect(
      program.methods
        .requestForExecution({
          amount: new BN(1),
          dstChain: 2,
          dstAddr: [
            ...Buffer.from(
              "0000000000000000000000000000000000000000000000000000000000000000",
              "hex",
            ),
          ],
          refundAddr: program.provider.publicKey!,
          signedQuoteBytes: Buffer.from(
            encodeSignedQuoteHeader(
              "EQ01",
              "0x0000000000000000000000000000000000000000",
              program.provider.publicKey!,
              1,
              2,
              BigInt(Date.now() + 1_000_000) / BigInt(1000),
              "0000000000002710000000003b9aca0700001d624add080000000625b3cb4600b69fffad8549dd87b875a85f6341283bc3cc61758e5b929cfa6913c8727af7a2776c66584cc260cb05d505c852187c0a29a7b774e1d05815020f774013b30fe11c",
            ),
          ),
          requestBytes: Buffer.from(
            "45524d310002000000000000000000000000157f9cd170058f373294addc32149f1f5c77a6410000000000000000000000910000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e11ba2b4d45eaed5996cd0823791e0c93114882d004f994e54540800000000000f42400000000000000000000000008738d8b87d770220aaf91239adc62a2ff3f88bbe000000000000000000000000e11ba2b4d45eaed5996cd0823791e0c93114882d0004",
            "hex",
          ),
          relayInstructions: Buffer.from(
            "0100000000000000000000000000030d4000000000000000000000000000000000",
            "hex",
          ),
        })
        .accounts({
          payee: program.provider.publicKey!,
        })
        .rpc(),
    ).to.be.fulfilled;
  });

  it("Pays the payee!", async () => {
    const payee = new anchor.web3.Keypair().publicKey;
    // the payee must already exist to avoid making a requestor pay to instantiate new accounts
    // if this is not done, the request will fail with
    // Message: Transaction simulation failed: Transaction results in an account (1) with insufficient funds for rent.
    const initialBalance = 1_000_000; // this has to be higher than rent exemption.
    {
      const p = anchor.getProvider();
      const tx = await p.connection.requestAirdrop(payee, initialBalance);
      await p.connection.confirmTransaction({
        ...(await p.connection.getLatestBlockhash()),
        signature: tx,
      });
    }
    const payment = 1000;
    await program.methods
      .requestForExecution({
        amount: new BN(payment),
        dstChain: 2,
        dstAddr: [
          ...Buffer.from(
            "0000000000000000000000000000000000000000000000000000000000000000",
            "hex",
          ),
        ],
        refundAddr: program.provider.publicKey!,
        signedQuoteBytes: Buffer.from(
          encodeSignedQuoteHeader(
            "EQ01",
            "0x0000000000000000000000000000000000000000",
            payee,
            1,
            2,
            BigInt(Date.now() + 1_000_000) / BigInt(1000),
          ),
        ),
        requestBytes: Buffer.from("", "hex"),
        relayInstructions: Buffer.from("", "hex"),
      })
      .accounts({ payee })
      .rpc();

    expect(
      await program.provider.connection.getBalance(payee, "processed"),
    ).to.equal(initialBalance + payment);
  });

  it("Reverts with src mismatch!", async () => {
    await expect(
      program.methods
        .requestForExecution({
          amount: new BN(1),
          dstChain: 2,
          dstAddr: [
            ...Buffer.from(
              "0000000000000000000000000000000000000000000000000000000000000000",
              "hex",
            ),
          ],
          refundAddr: program.provider.publicKey!,
          signedQuoteBytes: Buffer.from(
            encodeSignedQuoteHeader(
              "EQ01",
              "0x0000000000000000000000000000000000000000",
              program.provider.publicKey!,
              2,
              2,
              BigInt(Date.now() + 1_000_000) / BigInt(1000),
            ),
          ),
          requestBytes: Buffer.from("", "hex"),
          relayInstructions: Buffer.from("", "hex"),
        })
        .accounts({
          payee: program.provider.publicKey!,
        })
        .rpc(),
    ).to.be.rejectedWith(
      "Error Code: QuoteSrcChainMismatch. Error Number: 6001. Error Message: QuoteSrcChainMismatch.",
    );
  });

  it("Reverts with dst mismatch!", async () => {
    await expect(
      program.methods
        .requestForExecution({
          amount: new BN(1),
          dstChain: 2,
          dstAddr: [
            ...Buffer.from(
              "0000000000000000000000000000000000000000000000000000000000000000",
              "hex",
            ),
          ],
          refundAddr: program.provider.publicKey!,
          signedQuoteBytes: Buffer.from(
            encodeSignedQuoteHeader(
              "EQ01",
              "0x0000000000000000000000000000000000000000",
              program.provider.publicKey!,
              1,
              4,
              BigInt(Date.now() + 1_000_000) / BigInt(1000),
            ),
          ),
          requestBytes: Buffer.from("", "hex"),
          relayInstructions: Buffer.from("", "hex"),
        })
        .accounts({
          payee: program.provider.publicKey!,
        })
        .rpc(),
    ).to.be.rejectedWith(
      "Error Code: QuoteDstChainMismatch. Error Number: 6002. Error Message: QuoteDstChainMismatch.",
    );
  });

  it("Reverts with expired quote!", async () => {
    await expect(
      program.methods
        .requestForExecution({
          amount: new BN(1),
          dstChain: 2,
          dstAddr: [
            ...Buffer.from(
              "0000000000000000000000000000000000000000000000000000000000000000",
              "hex",
            ),
          ],
          refundAddr: program.provider.publicKey!,
          signedQuoteBytes: Buffer.from(
            encodeSignedQuoteHeader(
              "EQ01",
              "0x0000000000000000000000000000000000000000",
              program.provider.publicKey!,
              1,
              2,
              BigInt(Date.now() - 1_000_000) / BigInt(1000),
            ),
          ),
          requestBytes: Buffer.from("", "hex"),
          relayInstructions: Buffer.from("", "hex"),
        })
        .accounts({
          payee: program.provider.publicKey!,
        })
        .rpc(),
    ).to.be.rejectedWith(
      "Error Code: QuoteExpired. Error Number: 6003. Error Message: QuoteExpired.",
    );
  });

  it("Reverts with payee mismatch!", async () => {
    await expect(
      program.methods
        .requestForExecution({
          amount: new BN(1),
          dstChain: 2,
          dstAddr: [
            ...Buffer.from(
              "0000000000000000000000000000000000000000000000000000000000000000",
              "hex",
            ),
          ],
          refundAddr: program.provider.publicKey!,
          signedQuoteBytes: Buffer.from(
            encodeSignedQuoteHeader(
              "EQ01",
              "0x0000000000000000000000000000000000000000",
              program.provider.publicKey!,
              1,
              2,
              BigInt(Date.now() + 1_000_000) / BigInt(1000),
            ),
          ),
          requestBytes: Buffer.from("", "hex"),
          relayInstructions: Buffer.from("", "hex"),
        })
        .accounts({
          payee: new anchor.web3.Keypair().publicKey,
        })
        .rpc(),
    ).to.be.rejectedWith(
      "Error Code: QuotePayeeMismatch. Error Number: 6004. Error Message: QuotePayeeMismatch.",
    );
  });
});

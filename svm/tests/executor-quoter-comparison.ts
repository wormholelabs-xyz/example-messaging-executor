import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { ExecutorQuoterAnchor } from "../target/types/executor_quoter_anchor";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";

// Pinocchio program ID (from deployed keypair)
const PINOCCHIO_PROGRAM_ID = new PublicKey(
  "6yfXVhNgRKRk7YHFT8nTkVpFn5zXktbJddPUWK7jFAGX",
);

// Native program ID (from deployed keypair)
const NATIVE_PROGRAM_ID = new PublicKey(
  "9CFzEuwodz3UhfZeDpBqpRJGpnYLbBcADMTUEmXvGu42",
);

// Seeds for PDAs
const CONFIG_SEED = Buffer.from("config");
const CHAIN_INFO_SEED = Buffer.from("chain_info");
const QUOTE_SEED = Buffer.from("quote");

// Test chain configuration
const TEST_CHAIN_ID = 2; // Ethereum
const GAS_PRICE_DECIMALS = 15;
const NATIVE_DECIMALS = 18;

// Test quote values
const TEST_BASE_FEE = BigInt(100);
const TEST_SRC_PRICE = BigInt(2650000000); // SOL ~$265
const TEST_DST_PRICE = BigInt(160000000); // ETH ~$16 (for test)
const TEST_DST_GAS_PRICE = BigInt(399146);
const TEST_GAS_LIMIT = BigInt(250000);

interface GasResult {
  program: string;
  instruction: string;
  computeUnits: number;
}

describe("executor-quoter comparison", () => {
  // Create connection with confirmed commitment
  const connection = new Connection("http://127.0.0.1:8899", {
    commitment: "confirmed",
    confirmTransactionInitialTimeout: 60000,
  });

  // Load wallet from default location
  const walletPath =
    process.env.ANCHOR_WALLET || "/Users/smurf/.config/solana/id.json";
  const secretKey = JSON.parse(require("fs").readFileSync(walletPath, "utf8"));
  const payer = Keypair.fromSecretKey(new Uint8Array(secretKey));
  const wallet = new anchor.Wallet(payer);

  const provider = new anchor.AnchorProvider(connection, wallet, {
    commitment: "confirmed",
    preflightCommitment: "confirmed",
  });
  anchor.setProvider(provider);

  const anchorProgram = new Program<ExecutorQuoterAnchor>(
    require("../target/idl/executor_quoter_anchor.json"),
    provider,
  );
  const updater = Keypair.generate();

  // PDAs for Anchor program
  let anchorConfigPda: PublicKey;
  let anchorChainInfoPda: PublicKey;
  let anchorQuotePda: PublicKey;

  // PDAs for Pinocchio program
  let pinocchioConfigPda: PublicKey;
  let pinocchioConfigBump: number;
  let pinocchioChainInfoPda: PublicKey;
  let pinocchioChainInfoBump: number;
  let pinocchioQuotePda: PublicKey;
  let pinocchioQuoteBump: number;

  // PDAs for Native program
  let nativeConfigPda: PublicKey;
  let nativeConfigBump: number;
  let nativeChainInfoPda: PublicKey;
  let nativeChainInfoBump: number;
  let nativeQuotePda: PublicKey;
  let nativeQuoteBump: number;

  const gasResults: GasResult[] = [];

  // Helper to get compute units from transaction
  async function getComputeUnits(signature: string): Promise<number> {
    // Wait a bit for the tx to finalize
    await new Promise((resolve) => setTimeout(resolve, 500));
    const tx = await connection.getTransaction(signature, {
      commitment: "confirmed",
      maxSupportedTransactionVersion: 0,
    });
    if (!tx?.meta?.computeUnitsConsumed) {
      throw new Error("Could not get compute units");
    }
    return tx.meta.computeUnitsConsumed;
  }

  // Helper to build Pinocchio instruction data
  function buildPinocchioInitializeData(
    quoterAddress: PublicKey,
    updaterAddress: PublicKey,
    srcTokenDecimals: number,
    payeeAddress: Uint8Array,
    bump: number,
  ): Buffer {
    const data = Buffer.alloc(1 + 32 + 32 + 1 + 1 + 30 + 32); // discriminator + quoter + updater + decimals + bump + padding + payee
    let offset = 0;
    data.writeUInt8(0, offset); // Initialize discriminator
    offset += 1;
    quoterAddress.toBuffer().copy(data, offset);
    offset += 32;
    updaterAddress.toBuffer().copy(data, offset);
    offset += 32;
    data.writeUInt8(srcTokenDecimals, offset);
    offset += 1;
    data.writeUInt8(bump, offset);
    offset += 1;
    // padding (30 bytes)
    offset += 30;
    Buffer.from(payeeAddress).copy(data, offset);
    return data;
  }

  function buildPinocchioUpdateChainInfoData(
    chainId: number,
    enabled: number,
    gasPriceDecimals: number,
    nativeDecimals: number,
    bump: number,
  ): Buffer {
    // UpdateChainInfoData struct is 8 bytes (u16 + u8 + u8 + u8 + u8 + 2 bytes padding)
    const data = Buffer.alloc(1 + 8); // discriminator + struct
    let offset = 0;
    data.writeUInt8(1, offset); // UpdateChainInfo discriminator
    offset += 1;
    data.writeUInt16LE(chainId, offset);
    offset += 2;
    data.writeUInt8(enabled, offset);
    offset += 1;
    data.writeUInt8(gasPriceDecimals, offset);
    offset += 1;
    data.writeUInt8(nativeDecimals, offset);
    offset += 1;
    data.writeUInt8(bump, offset);
    offset += 1;
    // 2 bytes padding (already zeroed)
    return data;
  }

  function buildPinocchioUpdateQuoteData(
    chainId: number,
    dstPrice: bigint,
    srcPrice: bigint,
    dstGasPrice: bigint,
    baseFee: bigint,
    bump: number,
  ): Buffer {
    // UpdateQuoteData struct is 40 bytes (u16 + u8 bump + 5 bytes padding + 4 u64s)
    const data = Buffer.alloc(1 + 40); // discriminator + struct
    let offset = 0;
    data.writeUInt8(2, offset); // UpdateQuote discriminator
    offset += 1;
    data.writeUInt16LE(chainId, offset);
    offset += 2;
    data.writeUInt8(bump, offset);
    offset += 1;
    // 5 bytes padding (already zeroed)
    offset += 5;
    data.writeBigUInt64LE(dstPrice, offset);
    offset += 8;
    data.writeBigUInt64LE(srcPrice, offset);
    offset += 8;
    data.writeBigUInt64LE(dstGasPrice, offset);
    offset += 8;
    data.writeBigUInt64LE(baseFee, offset);
    return data;
  }

  function buildPinocchioRequestQuoteData(
    dstChain: number,
    dstAddr: Uint8Array,
    refundAddr: Uint8Array,
    requestBytes: Uint8Array,
    relayInstructions: Uint8Array,
  ): Buffer {
    const data = Buffer.alloc(
      1 + 2 + 32 + 32 + 4 + requestBytes.length + 4 + relayInstructions.length,
    );
    let offset = 0;
    data.writeUInt8(3, offset); // RequestQuote discriminator
    offset += 1;
    data.writeUInt16LE(dstChain, offset);
    offset += 2;
    Buffer.from(dstAddr).copy(data, offset);
    offset += 32;
    Buffer.from(refundAddr).copy(data, offset);
    offset += 32;
    data.writeUInt32LE(requestBytes.length, offset);
    offset += 4;
    Buffer.from(requestBytes).copy(data, offset);
    offset += requestBytes.length;
    data.writeUInt32LE(relayInstructions.length, offset);
    offset += 4;
    Buffer.from(relayInstructions).copy(data, offset);
    return data;
  }

  function buildRelayInstructions(gasLimit: bigint, msgValue: bigint): Buffer {
    // Type 1 (Gas): 1 byte type + 16 bytes gas_limit + 16 bytes msg_value
    const data = Buffer.alloc(33);
    data.writeUInt8(1, 0); // IX_TYPE_GAS
    // Write gas_limit as big-endian u128
    const gasLimitBuf = Buffer.alloc(16);
    gasLimitBuf.writeBigUInt64BE(gasLimit >> BigInt(64), 0);
    gasLimitBuf.writeBigUInt64BE(gasLimit & BigInt("0xFFFFFFFFFFFFFFFF"), 8);
    gasLimitBuf.copy(data, 1);
    // Write msg_value as big-endian u128
    const msgValueBuf = Buffer.alloc(16);
    msgValueBuf.writeBigUInt64BE(msgValue >> BigInt(64), 0);
    msgValueBuf.writeBigUInt64BE(msgValue & BigInt("0xFFFFFFFFFFFFFFFF"), 8);
    msgValueBuf.copy(data, 17);
    return data;
  }

  before(async () => {
    console.log(`Payer pubkey: ${payer.publicKey.toBase58()}`);

    // Airdrop to payer
    const payerAirdropSig = await connection.requestAirdrop(
      payer.publicKey,
      100 * anchor.web3.LAMPORTS_PER_SOL,
    );
    await connection.confirmTransaction(payerAirdropSig, "confirmed");
    console.log(
      `Payer balance: ${await connection.getBalance(payer.publicKey)}`,
    );

    // Airdrop to updater
    const sig = await connection.requestAirdrop(
      updater.publicKey,
      10 * anchor.web3.LAMPORTS_PER_SOL,
    );
    await connection.confirmTransaction(sig, "confirmed");

    // Derive Anchor PDAs
    [anchorConfigPda] = PublicKey.findProgramAddressSync(
      [CONFIG_SEED],
      anchorProgram.programId,
    );
    const chainIdBytes = Buffer.alloc(2);
    chainIdBytes.writeUInt16LE(TEST_CHAIN_ID);
    [anchorChainInfoPda] = PublicKey.findProgramAddressSync(
      [CHAIN_INFO_SEED, chainIdBytes],
      anchorProgram.programId,
    );
    [anchorQuotePda] = PublicKey.findProgramAddressSync(
      [QUOTE_SEED, chainIdBytes],
      anchorProgram.programId,
    );

    // Derive Pinocchio PDAs
    [pinocchioConfigPda, pinocchioConfigBump] =
      PublicKey.findProgramAddressSync([CONFIG_SEED], PINOCCHIO_PROGRAM_ID);
    [pinocchioChainInfoPda, pinocchioChainInfoBump] =
      PublicKey.findProgramAddressSync(
        [CHAIN_INFO_SEED, chainIdBytes],
        PINOCCHIO_PROGRAM_ID,
      );
    [pinocchioQuotePda, pinocchioQuoteBump] = PublicKey.findProgramAddressSync(
      [QUOTE_SEED, chainIdBytes],
      PINOCCHIO_PROGRAM_ID,
    );

    // Derive Native PDAs
    [nativeConfigPda, nativeConfigBump] = PublicKey.findProgramAddressSync(
      [CONFIG_SEED],
      NATIVE_PROGRAM_ID,
    );
    [nativeChainInfoPda, nativeChainInfoBump] =
      PublicKey.findProgramAddressSync(
        [CHAIN_INFO_SEED, chainIdBytes],
        NATIVE_PROGRAM_ID,
      );
    [nativeQuotePda, nativeQuoteBump] = PublicKey.findProgramAddressSync(
      [QUOTE_SEED, chainIdBytes],
      NATIVE_PROGRAM_ID,
    );
  });

  describe("Anchor Implementation", () => {
    it("initializes config", async () => {
      const payeeAddress = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(payeeAddress);

      const sig = await anchorProgram.methods
        .initialize({
          quoterAddress: payer.publicKey,
          updaterAddress: updater.publicKey,
          srcTokenDecimals: 9, // SOL
          payeeAddress: Array.from(payeeAddress),
        })
        .accounts({
          payer: payer.publicKey,
          config: anchorConfigPda,
          systemProgram: SystemProgram.programId,
        })
        .rpc({ commitment: "confirmed" });

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Anchor",
        instruction: "initialize",
        computeUnits,
      });

      console.log(`  Anchor initialize: ${computeUnits} CU`);
    });

    it("updates chain info", async () => {
      const sig = await anchorProgram.methods
        .updateChainInfo({
          chainId: TEST_CHAIN_ID,
          enabled: true,
          gasPriceDecimals: GAS_PRICE_DECIMALS,
          nativeDecimals: NATIVE_DECIMALS,
        })
        .accounts({
          payer: payer.publicKey,
          updater: updater.publicKey,
          config: anchorConfigPda,
          chainInfo: anchorChainInfoPda,
          systemProgram: SystemProgram.programId,
        })
        .signers([updater])
        .rpc({ commitment: "confirmed" });

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Anchor",
        instruction: "updateChainInfo",
        computeUnits,
      });

      console.log(`  Anchor updateChainInfo: ${computeUnits} CU`);
    });

    it("updates quote", async () => {
      const sig = await anchorProgram.methods
        .updateQuote({
          chainId: TEST_CHAIN_ID,
          dstPrice: new anchor.BN(TEST_DST_PRICE.toString()),
          srcPrice: new anchor.BN(TEST_SRC_PRICE.toString()),
          dstGasPrice: new anchor.BN(TEST_DST_GAS_PRICE.toString()),
          baseFee: new anchor.BN(TEST_BASE_FEE.toString()),
        })
        .accounts({
          payer: payer.publicKey,
          updater: updater.publicKey,
          config: anchorConfigPda,
          quoteBody: anchorQuotePda,
          systemProgram: SystemProgram.programId,
        })
        .signers([updater])
        .rpc({ commitment: "confirmed" });

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Anchor",
        instruction: "updateQuote",
        computeUnits,
      });

      console.log(`  Anchor updateQuote: ${computeUnits} CU`);
    });

    it("requests quote", async () => {
      const dstAddr = new Uint8Array(32);
      const refundAddr = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(refundAddr);
      const relayInstructions = buildRelayInstructions(
        TEST_GAS_LIMIT,
        BigInt(0),
      );

      const sig = await anchorProgram.methods
        .requestQuote({
          dstChain: TEST_CHAIN_ID,
          dstAddr: Array.from(dstAddr),
          refundAddr: Array.from(refundAddr),
          requestBytes: Buffer.alloc(0),
          relayInstructions: relayInstructions,
        })
        .accounts({
          config: anchorConfigPda,
          chainInfo: anchorChainInfoPda,
          quoteBody: anchorQuotePda,
        })
        .rpc({ commitment: "confirmed" });

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Anchor",
        instruction: "requestQuote",
        computeUnits,
      });

      console.log(`  Anchor requestQuote: ${computeUnits} CU`);
    });

    it("requests execution quote", async () => {
      const dstAddr = new Uint8Array(32);
      const refundAddr = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(refundAddr);
      const relayInstructions = buildRelayInstructions(
        TEST_GAS_LIMIT,
        BigInt(0),
      );

      const sig = await anchorProgram.methods
        .requestExecutionQuote({
          dstChain: TEST_CHAIN_ID,
          dstAddr: Array.from(dstAddr),
          refundAddr: Array.from(refundAddr),
          requestBytes: Buffer.alloc(0),
          relayInstructions: relayInstructions,
        })
        .accounts({
          config: anchorConfigPda,
          chainInfo: anchorChainInfoPda,
          quoteBody: anchorQuotePda,
        })
        .rpc({ commitment: "confirmed" });

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Anchor",
        instruction: "requestExecutionQuote",
        computeUnits,
      });

      console.log(`  Anchor requestExecutionQuote: ${computeUnits} CU`);
    });
  });

  describe("Pinocchio Implementation", () => {
    it("initializes config", async () => {
      const payeeAddress = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(payeeAddress);

      const data = buildPinocchioInitializeData(
        payer.publicKey,
        updater.publicKey,
        9,
        payeeAddress,
        pinocchioConfigBump,
      );

      const ix = new TransactionInstruction({
        programId: PINOCCHIO_PROGRAM_ID,
        keys: [
          { pubkey: payer.publicKey, isSigner: true, isWritable: true },
          { pubkey: pinocchioConfigPda, isSigner: false, isWritable: true },
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer],
        {
          commitment: "confirmed",
        },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Pinocchio",
        instruction: "initialize",
        computeUnits,
      });

      console.log(`  Pinocchio initialize: ${computeUnits} CU`);
    });

    it("updates chain info", async () => {
      const data = buildPinocchioUpdateChainInfoData(
        TEST_CHAIN_ID,
        1, // enabled
        GAS_PRICE_DECIMALS,
        NATIVE_DECIMALS,
        pinocchioChainInfoBump,
      );

      const ix = new TransactionInstruction({
        programId: PINOCCHIO_PROGRAM_ID,
        keys: [
          { pubkey: payer.publicKey, isSigner: true, isWritable: true },
          { pubkey: updater.publicKey, isSigner: true, isWritable: false },
          { pubkey: pinocchioConfigPda, isSigner: false, isWritable: false },
          { pubkey: pinocchioChainInfoPda, isSigner: false, isWritable: true },
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer, updater],
        { commitment: "confirmed" },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Pinocchio",
        instruction: "updateChainInfo",
        computeUnits,
      });

      console.log(`  Pinocchio updateChainInfo: ${computeUnits} CU`);
    });

    it("updates quote", async () => {
      const data = buildPinocchioUpdateQuoteData(
        TEST_CHAIN_ID,
        TEST_DST_PRICE,
        TEST_SRC_PRICE,
        TEST_DST_GAS_PRICE,
        TEST_BASE_FEE,
        pinocchioQuoteBump,
      );

      const ix = new TransactionInstruction({
        programId: PINOCCHIO_PROGRAM_ID,
        keys: [
          { pubkey: payer.publicKey, isSigner: true, isWritable: true },
          { pubkey: updater.publicKey, isSigner: true, isWritable: false },
          { pubkey: pinocchioConfigPda, isSigner: false, isWritable: false },
          { pubkey: pinocchioQuotePda, isSigner: false, isWritable: true },
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer, updater],
        { commitment: "confirmed" },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Pinocchio",
        instruction: "updateQuote",
        computeUnits,
      });

      console.log(`  Pinocchio updateQuote: ${computeUnits} CU`);
    });

    it("requests quote", async () => {
      const dstAddr = new Uint8Array(32);
      const refundAddr = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(refundAddr);
      const relayInstructions = buildRelayInstructions(
        TEST_GAS_LIMIT,
        BigInt(0),
      );

      const data = buildPinocchioRequestQuoteData(
        TEST_CHAIN_ID,
        dstAddr,
        refundAddr,
        new Uint8Array(0),
        relayInstructions,
      );

      const ix = new TransactionInstruction({
        programId: PINOCCHIO_PROGRAM_ID,
        keys: [
          { pubkey: pinocchioConfigPda, isSigner: false, isWritable: false },
          { pubkey: pinocchioChainInfoPda, isSigner: false, isWritable: false },
          { pubkey: pinocchioQuotePda, isSigner: false, isWritable: false },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer],
        {
          commitment: "confirmed",
        },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Pinocchio",
        instruction: "requestQuote",
        computeUnits,
      });

      console.log(`  Pinocchio requestQuote: ${computeUnits} CU`);
    });

    it("requests execution quote", async () => {
      const dstAddr = new Uint8Array(32);
      const refundAddr = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(refundAddr);
      const relayInstructions = buildRelayInstructions(
        TEST_GAS_LIMIT,
        BigInt(0),
      );

      // RequestExecutionQuote is discriminator 4
      const data = buildPinocchioRequestQuoteData(
        TEST_CHAIN_ID,
        dstAddr,
        refundAddr,
        new Uint8Array(0),
        relayInstructions,
      );
      data.writeUInt8(4, 0); // Change discriminator to RequestExecutionQuote

      const ix = new TransactionInstruction({
        programId: PINOCCHIO_PROGRAM_ID,
        keys: [
          { pubkey: pinocchioConfigPda, isSigner: false, isWritable: false },
          { pubkey: pinocchioChainInfoPda, isSigner: false, isWritable: false },
          { pubkey: pinocchioQuotePda, isSigner: false, isWritable: false },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer],
        {
          commitment: "confirmed",
        },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Pinocchio",
        instruction: "requestExecutionQuote",
        computeUnits,
      });

      console.log(`  Pinocchio requestExecutionQuote: ${computeUnits} CU`);
    });
  });

  describe("Native Implementation", () => {
    it("initializes config", async () => {
      const payeeAddress = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(payeeAddress);

      const data = buildPinocchioInitializeData(
        payer.publicKey,
        updater.publicKey,
        9,
        payeeAddress,
        nativeConfigBump,
      );

      const ix = new TransactionInstruction({
        programId: NATIVE_PROGRAM_ID,
        keys: [
          { pubkey: payer.publicKey, isSigner: true, isWritable: true },
          { pubkey: nativeConfigPda, isSigner: false, isWritable: true },
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer],
        {
          commitment: "confirmed",
        },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Native",
        instruction: "initialize",
        computeUnits,
      });

      console.log(`  Native initialize: ${computeUnits} CU`);
    });

    it("updates chain info", async () => {
      const data = buildPinocchioUpdateChainInfoData(
        TEST_CHAIN_ID,
        1, // enabled
        GAS_PRICE_DECIMALS,
        NATIVE_DECIMALS,
        nativeChainInfoBump,
      );

      const ix = new TransactionInstruction({
        programId: NATIVE_PROGRAM_ID,
        keys: [
          { pubkey: payer.publicKey, isSigner: true, isWritable: true },
          { pubkey: updater.publicKey, isSigner: true, isWritable: false },
          { pubkey: nativeConfigPda, isSigner: false, isWritable: false },
          { pubkey: nativeChainInfoPda, isSigner: false, isWritable: true },
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer, updater],
        { commitment: "confirmed" },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Native",
        instruction: "updateChainInfo",
        computeUnits,
      });

      console.log(`  Native updateChainInfo: ${computeUnits} CU`);
    });

    it("updates quote", async () => {
      const data = buildPinocchioUpdateQuoteData(
        TEST_CHAIN_ID,
        TEST_DST_PRICE,
        TEST_SRC_PRICE,
        TEST_DST_GAS_PRICE,
        TEST_BASE_FEE,
        nativeQuoteBump,
      );

      const ix = new TransactionInstruction({
        programId: NATIVE_PROGRAM_ID,
        keys: [
          { pubkey: payer.publicKey, isSigner: true, isWritable: true },
          { pubkey: updater.publicKey, isSigner: true, isWritable: false },
          { pubkey: nativeConfigPda, isSigner: false, isWritable: false },
          { pubkey: nativeQuotePda, isSigner: false, isWritable: true },
          {
            pubkey: SystemProgram.programId,
            isSigner: false,
            isWritable: false,
          },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer, updater],
        { commitment: "confirmed" },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Native",
        instruction: "updateQuote",
        computeUnits,
      });

      console.log(`  Native updateQuote: ${computeUnits} CU`);
    });

    it("requests quote", async () => {
      const dstAddr = new Uint8Array(32);
      const refundAddr = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(refundAddr);
      const relayInstructions = buildRelayInstructions(
        TEST_GAS_LIMIT,
        BigInt(0),
      );

      const data = buildPinocchioRequestQuoteData(
        TEST_CHAIN_ID,
        dstAddr,
        refundAddr,
        new Uint8Array(0),
        relayInstructions,
      );

      const ix = new TransactionInstruction({
        programId: NATIVE_PROGRAM_ID,
        keys: [
          { pubkey: nativeConfigPda, isSigner: false, isWritable: false },
          { pubkey: nativeChainInfoPda, isSigner: false, isWritable: false },
          { pubkey: nativeQuotePda, isSigner: false, isWritable: false },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer],
        {
          commitment: "confirmed",
        },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Native",
        instruction: "requestQuote",
        computeUnits,
      });

      console.log(`  Native requestQuote: ${computeUnits} CU`);
    });

    it("requests execution quote", async () => {
      const dstAddr = new Uint8Array(32);
      const refundAddr = new Uint8Array(32);
      payer.publicKey.toBuffer().copy(refundAddr);
      const relayInstructions = buildRelayInstructions(
        TEST_GAS_LIMIT,
        BigInt(0),
      );

      // RequestExecutionQuote is discriminator 4
      const data = buildPinocchioRequestQuoteData(
        TEST_CHAIN_ID,
        dstAddr,
        refundAddr,
        new Uint8Array(0),
        relayInstructions,
      );
      data.writeUInt8(4, 0); // Change discriminator to RequestExecutionQuote

      const ix = new TransactionInstruction({
        programId: NATIVE_PROGRAM_ID,
        keys: [
          { pubkey: nativeConfigPda, isSigner: false, isWritable: false },
          { pubkey: nativeChainInfoPda, isSigner: false, isWritable: false },
          { pubkey: nativeQuotePda, isSigner: false, isWritable: false },
        ],
        data,
      });

      const tx = new Transaction().add(ix);
      const sig = await sendAndConfirmTransaction(
        provider.connection,
        tx,
        [payer],
        {
          commitment: "confirmed",
        },
      );

      const computeUnits = await getComputeUnits(sig);
      gasResults.push({
        program: "Native",
        instruction: "requestExecutionQuote",
        computeUnits,
      });

      console.log(`  Native requestExecutionQuote: ${computeUnits} CU`);
    });
  });

  describe("Gas Comparison Summary", () => {
    it("prints comparison table", () => {
      console.log("\n=== Gas Comparison (Compute Units) ===\n");
      console.log(
        "| Instruction            | Anchor CU | Native CU | Pinocchio CU | Pino vs Anchor | Pino vs Native |",
      );
      console.log(
        "|------------------------|-----------|-----------|--------------|----------------|----------------|",
      );

      const instructions = [
        "initialize",
        "updateChainInfo",
        "updateQuote",
        "requestQuote",
        "requestExecutionQuote",
      ];

      for (const instruction of instructions) {
        const anchor = gasResults.find(
          (r) => r.program === "Anchor" && r.instruction === instruction,
        );
        const native = gasResults.find(
          (r) => r.program === "Native" && r.instruction === instruction,
        );
        const pinocchio = gasResults.find(
          (r) => r.program === "Pinocchio" && r.instruction === instruction,
        );

        if (anchor && native && pinocchio) {
          const pinoVsAnchor = (
            (1 - pinocchio.computeUnits / anchor.computeUnits) *
            100
          ).toFixed(1);
          const pinoVsNative = (
            (1 - pinocchio.computeUnits / native.computeUnits) *
            100
          ).toFixed(1);
          console.log(
            `| ${instruction.padEnd(22)} | ${anchor.computeUnits.toString().padStart(9)} | ${native.computeUnits.toString().padStart(9)} | ${pinocchio.computeUnits.toString().padStart(12)} | ${pinoVsAnchor.padStart(13)}% | ${pinoVsNative.padStart(13)}% |`,
          );
        }
      }

      console.log("\n=== Binary Size Comparison ===\n");
      console.log("| Program    | Size (bytes) | vs Pinocchio |");
      console.log("|------------|--------------|--------------|");
      console.log("| Anchor     |       277728 |        5.32x |");
      console.log("| Native     |       113504 |        2.17x |");
      console.log("| Pinocchio  |        52224 |   1x (base)  |");
    });
  });
});

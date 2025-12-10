/**
 * Integration tests for executor-quoter and executor-quoter-router.
 *
 * Usage: bun test ./tests/executor-quoters.test.ts
 */

import { beforeAll, describe, expect, test } from "bun:test";
import {
  Connection,
  Keypair,
  PublicKey,
  SystemProgram,
  Transaction,
  TransactionInstruction,
  TransactionMessage,
  VersionedTransaction,
  sendAndConfirmTransaction,
} from "@solana/web3.js";
import { signAsync, getPublicKey, utils } from "@noble/secp256k1";
import { keccak256 } from "js-sha3";
import * as fs from "fs";

// Program IDs (deployed to devnet)
const QUOTER_PROGRAM_ID = new PublicKey("957QU51h6VLbnbAmAPgtXz1kbFE1QhchDmNfgugW9xCc");
const ROUTER_PROGRAM_ID = new PublicKey("5pkyS8pnbbMforEqAR91gkgPeBs5XKhWpiuuuLdw6hbk");
const EXECUTOR_PROGRAM_ID = new PublicKey("execXUrAsMnqMmTHj5m7N1YQgsDz3cwGLYCYyuDRciV");

// Instruction discriminators for executor-quoter
// Instructions 0-1 use 1-byte discriminator, 2-3 use 8-byte discriminator
const IX_QUOTER_UPDATE_CHAIN_INFO = 0;
const IX_QUOTER_UPDATE_QUOTE = 1;
const IX_QUOTER_REQUEST_QUOTE = 2;  // Uses 8-byte discriminator

// Instruction discriminators for executor-quoter-router
const IX_ROUTER_UPDATE_QUOTER_CONTRACT = 0;
const IX_ROUTER_QUOTE_EXECUTION = 1;

// Chain IDs
const CHAIN_ID_SOLANA = 1;
const CHAIN_ID_ETHEREUM = 2;

// Testnet chain IDs
const CHAIN_ID_ETH_SEPOLIA = 10002;
const CHAIN_ID_ARBITRUM_SEPOLIA = 10003;
const CHAIN_ID_BASE_SEPOLIA = 10004;
const CHAIN_ID_OPTIMISM_SEPOLIA = 10005;

// Quote calculation constants (must match Rust)
const QUOTE_DECIMALS = 10n;
const SVM_DECIMAL_RESOLUTION = 9n;
const EVM_DECIMAL_RESOLUTION = 18n;

// Test quote parameters for mainnet Ethereum
const TEST_DST_PRICE = 3000n * 10n ** QUOTE_DECIMALS;  // ETH $3000
const TEST_SRC_PRICE = 200n * 10n ** QUOTE_DECIMALS;   // SOL $200
const TEST_DST_GAS_PRICE = 20n;                         // 20 gwei (decimals=9)
const TEST_BASE_FEE = 1_000_000n;                       // 0.001 SOL in lamports
const TEST_GAS_PRICE_DECIMALS = 9;                      // gwei decimals
const TEST_NATIVE_DECIMALS = 18;                        // ETH decimals

// Testnet chain configurations with realistic gas prices
// All testnets use ETH as native token (18 decimals)
// Gas prices vary by L2 characteristics
interface ChainConfig {
  chainId: number;
  name: string;
  dstPrice: bigint;        // Native token price in QUOTE_DECIMALS
  gasPriceDecimals: number;
  nativeDecimals: number;
  dstGasPrice: bigint;     // Gas price in gasPriceDecimals
  baseFee: bigint;         // Base fee in lamports
}

const TESTNET_CHAINS: ChainConfig[] = [
  {
    // Ethereum Sepolia - standard L1 gas prices
    // All EVM chains store gas price in wei (gasPriceDecimals=18)
    // Reference: w7-executor/src/env/testnet.ts
    chainId: CHAIN_ID_ETH_SEPOLIA,
    name: "Ethereum Sepolia",
    dstPrice: 3000n * 10n ** QUOTE_DECIMALS,  // ETH ~$3000
    gasPriceDecimals: 18,                      // gas price stored in wei
    nativeDecimals: 18,
    dstGasPrice: 25_000_000_000n,              // 25 gwei in wei
    baseFee: 1_000_000n,                       // 0.001 SOL
  },
  {
    // Arbitrum Sepolia - L2 with lower gas prices
    // Arbitrum min gas price floor is 0.01 gwei
    chainId: CHAIN_ID_ARBITRUM_SEPOLIA,
    name: "Arbitrum Sepolia",
    dstPrice: 3000n * 10n ** QUOTE_DECIMALS,  // ETH ~$3000
    gasPriceDecimals: 18,                      // gas price stored in wei
    nativeDecimals: 18,
    dstGasPrice: 100_000_000n,                 // 0.1 gwei in wei
    baseFee: 500_000n,                         // 0.0005 SOL (lower for L2)
  },
  {
    // Base Sepolia - L2 with very low gas prices
    // Base typically has gas prices around 0.001-0.01 gwei
    chainId: CHAIN_ID_BASE_SEPOLIA,
    name: "Base Sepolia",
    dstPrice: 3000n * 10n ** QUOTE_DECIMALS,  // ETH ~$3000
    gasPriceDecimals: 18,                      // gas price stored in wei
    nativeDecimals: 18,
    dstGasPrice: 10_000_000n,                  // 0.01 gwei in wei
    baseFee: 500_000n,                         // 0.0005 SOL
  },
  {
    // Optimism Sepolia - L2 with low gas prices
    // OP typically has gas prices around 0.001-0.05 gwei
    chainId: CHAIN_ID_OPTIMISM_SEPOLIA,
    name: "Optimism Sepolia",
    dstPrice: 3000n * 10n ** QUOTE_DECIMALS,  // ETH ~$3000
    gasPriceDecimals: 18,                      // gas price stored in wei
    nativeDecimals: 18,
    dstGasPrice: 50_000_000n,                  // 0.05 gwei in wei
    baseFee: 500_000n,                         // 0.0005 SOL
  },
];

// Test request parameters
const TEST_GAS_LIMIT = 100_000n;
const TEST_MSG_VALUE = 10n ** 18n; // 1 ETH in wei

/**
 * Calculate expected quote using the same algorithm as the Rust implementation.
 * Returns the expected payment in lamports.
 */
function calculateExpectedQuote(
  baseFee: bigint,
  srcPrice: bigint,
  dstPrice: bigint,
  dstGasPrice: bigint,
  gasPriceDecimals: number,
  nativeDecimals: number,
  gasLimit: bigint,
  msgValue: bigint,
): bigint {
  const pow10 = (exp: bigint) => 10n ** exp;

  // Normalize helper
  const normalize = (amount: bigint, from: bigint, to: bigint): bigint => {
    if (from > to) return amount / pow10(from - to);
    if (from < to) return amount * pow10(to - from);
    return amount;
  };

  // mul_decimals: (a * b) / 10^decimals
  const mulDecimals = (a: bigint, b: bigint, decimals: bigint): bigint => {
    return (a * b) / pow10(decimals);
  };

  // div_decimals: (a * 10^decimals) / b
  const divDecimals = (a: bigint, b: bigint, decimals: bigint): bigint => {
    return (a * pow10(decimals)) / b;
  };

  // 1. Base fee conversion
  const srcChainValueForBaseFee = normalize(baseFee, QUOTE_DECIMALS, EVM_DECIMAL_RESOLUTION);

  // 2. Price ratio
  const nSrcPrice = normalize(srcPrice, QUOTE_DECIMALS, EVM_DECIMAL_RESOLUTION);
  const nDstPrice = normalize(dstPrice, QUOTE_DECIMALS, EVM_DECIMAL_RESOLUTION);
  const scaledConversion = divDecimals(nDstPrice, nSrcPrice, EVM_DECIMAL_RESOLUTION);

  // 3. Gas limit cost
  const gasCost = gasLimit * dstGasPrice;
  const nGasLimitCost = normalize(gasCost, BigInt(gasPriceDecimals), EVM_DECIMAL_RESOLUTION);
  const srcChainValueForGasLimit = mulDecimals(nGasLimitCost, scaledConversion, EVM_DECIMAL_RESOLUTION);

  // 4. Message value conversion
  const nMsgValue = normalize(msgValue, BigInt(nativeDecimals), EVM_DECIMAL_RESOLUTION);
  const srcChainValueForMsgValue = mulDecimals(nMsgValue, scaledConversion, EVM_DECIMAL_RESOLUTION);

  // 5. Sum (in EVM decimals)
  const totalEvm = srcChainValueForBaseFee + srcChainValueForGasLimit + srcChainValueForMsgValue;

  // 6. Convert to SVM decimals (lamports)
  return normalize(totalEvm, EVM_DECIMAL_RESOLUTION, SVM_DECIMAL_RESOLUTION);
}

// Helpers

function keccak256Hash(data: Uint8Array): Uint8Array {
  const hex = keccak256(data);
  const result = new Uint8Array(32);
  for (let i = 0; i < 32; i++) {
    result[i] = parseInt(hex.substring(i * 2, i * 2 + 2), 16);
  }
  return result;
}

function loadWallet(): Keypair {
  const path = process.env.WALLET_PATH;
  if (!path) {
    throw new Error("WALLET_PATH environment variable is required");
  }
  const secretKey = JSON.parse(fs.readFileSync(path, "utf-8"));
  return Keypair.fromSecretKey(Uint8Array.from(secretKey));
}

// PDA derivation

function deriveQuoterConfigPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([Buffer.from("config")], QUOTER_PROGRAM_ID);
}

function deriveQuoterChainInfoPda(chainId: number): [PublicKey, number] {
  const buf = Buffer.alloc(2);
  buf.writeUInt16LE(chainId);
  return PublicKey.findProgramAddressSync([Buffer.from("chain_info"), buf], QUOTER_PROGRAM_ID);
}

function deriveQuoterQuoteBodyPda(chainId: number): [PublicKey, number] {
  const buf = Buffer.alloc(2);
  buf.writeUInt16LE(chainId);
  return PublicKey.findProgramAddressSync([Buffer.from("quote"), buf], QUOTER_PROGRAM_ID);
}

function deriveRouterConfigPda(): [PublicKey, number] {
  return PublicKey.findProgramAddressSync([Buffer.from("config")], ROUTER_PROGRAM_ID);
}

function deriveQuoterRegistrationPda(quoterAddress: Uint8Array): [PublicKey, number] {
  return PublicKey.findProgramAddressSync(
    [Buffer.from("quoter_registration"), quoterAddress],
    ROUTER_PROGRAM_ID
  );
}

// Secp256k1 identity for governance signing

class QuoterIdentity {
  privateKey: Uint8Array;
  publicKey: Uint8Array;
  ethAddress: Uint8Array;

  constructor() {
    this.privateKey = utils.randomSecretKey();
    this.publicKey = getPublicKey(this.privateKey, false);
    const pubkeyHash = keccak256Hash(this.publicKey.slice(1));
    this.ethAddress = pubkeyHash.slice(12);
  }

  async sign(message: Uint8Array): Promise<{ r: Uint8Array; s: Uint8Array; v: number }> {
    const msgHash = keccak256Hash(message);
    const sig = await signAsync(msgHash, this.privateKey, {
      lowS: true,
      extraEntropy: false,
      prehash: false,
      format: "recovered",
    }) as Uint8Array;
    return {
      r: sig.slice(1, 33),
      s: sig.slice(33, 65),
      v: sig[0] + 27,
    };
  }
}

// Instruction builders

/**
 * Build UpdateChainInfo instruction data.
 * Layout: discriminator (1) + chain_id (2) + enabled (1) + gas_price_decimals (1) + native_decimals (1) + padding (1)
 */
function buildUpdateChainInfoData(
  chainId: number,
  enabled: number,
  gasPriceDecimals: number,
  nativeDecimals: number,
): Buffer {
  const data = Buffer.alloc(7);
  let o = 0;
  data.writeUInt8(IX_QUOTER_UPDATE_CHAIN_INFO, o++);
  data.writeUInt16LE(chainId, o); o += 2;
  data.writeUInt8(enabled, o++);
  data.writeUInt8(gasPriceDecimals, o++);
  data.writeUInt8(nativeDecimals, o++);
  data.writeUInt8(0, o); // padding
  return data;
}

/**
 * Build UpdateQuote instruction data.
 * Layout: discriminator (1) + chain_id (2) + padding (6) + dst_price (8) + src_price (8) + dst_gas_price (8) + base_fee (8)
 */
function buildUpdateQuoteData(
  chainId: number,
  dstPrice: bigint,
  srcPrice: bigint,
  dstGasPrice: bigint,
  baseFee: bigint
): Buffer {
  const data = Buffer.alloc(41);
  let o = 0;
  data.writeUInt8(IX_QUOTER_UPDATE_QUOTE, o++);
  data.writeUInt16LE(chainId, o); o += 2;
  o += 6; // padding (6 bytes to align to 8-byte boundary for u64s)
  data.writeBigUInt64LE(dstPrice, o); o += 8;
  data.writeBigUInt64LE(srcPrice, o); o += 8;
  data.writeBigUInt64LE(dstGasPrice, o); o += 8;
  data.writeBigUInt64LE(baseFee, o);
  return data;
}

/**
 * Build UpdateQuoterContract instruction data.
 * Layout: discriminator (1) + governance_message (163)
 *
 * Governance message (163 bytes):
 * - prefix "EG01" (4)
 * - chain_id (2, BE)
 * - quoter_address (20)
 * - universal_contract_address (32)
 * - universal_sender_address (32)
 * - expiry_time (8, BE)
 * - signature_r (32)
 * - signature_s (32)
 * - signature_v (1)
 */
async function buildUpdateQuoterContractData(
  quoter: QuoterIdentity,
  implementationProgramId: PublicKey,
  sender: PublicKey,
  chainId: number,
  expiryTime: bigint,
): Promise<Buffer> {
  // Build the message body (98 bytes - everything before signature)
  const body = Buffer.alloc(98);
  let o = 0;
  Buffer.from("EG01").copy(body, o); o += 4;
  body.writeUInt16BE(chainId, o); o += 2;
  Buffer.from(quoter.ethAddress).copy(body, o); o += 20;
  implementationProgramId.toBuffer().copy(body, o); o += 32;
  sender.toBuffer().copy(body, o); o += 32;
  body.writeBigUInt64BE(expiryTime, o);

  // Sign the body
  const { r, s, v } = await quoter.sign(body);

  // Build full instruction data: discriminator + governance message
  const data = Buffer.alloc(164);
  o = 0;
  data.writeUInt8(IX_ROUTER_UPDATE_QUOTER_CONTRACT, o++);
  body.copy(data, o); o += 98;
  Buffer.from(r).copy(data, o); o += 32;
  Buffer.from(s).copy(data, o); o += 32;
  data.writeUInt8(v, o);
  return data;
}

/**
 * Build RequestQuote instruction data.
 * Uses 8-byte discriminator for Anchor compatibility.
 * Layout: discriminator (8) + dst_chain (2) + dst_addr (32) + refund_addr (32) +
 *         request_bytes_len (4) + request_bytes + relay_instructions_len (4) + relay_instructions
 */
function buildRequestQuoteData(
  dstChain: number,
  dstAddr: Uint8Array,
  refundAddr: Uint8Array,
  requestBytes: Uint8Array,
  relayInstructions: Buffer
): Buffer {
  const data = Buffer.alloc(8 + 2 + 32 + 32 + 4 + requestBytes.length + 4 + relayInstructions.length);
  let o = 0;
  // 8-byte discriminator: instruction ID in first byte, rest zeros
  data.writeUInt8(IX_QUOTER_REQUEST_QUOTE, o++);
  o += 7; // padding for 8-byte discriminator
  data.writeUInt16LE(dstChain, o); o += 2;
  Buffer.from(dstAddr).copy(data, o); o += 32;
  Buffer.from(refundAddr).copy(data, o); o += 32;
  data.writeUInt32LE(requestBytes.length, o); o += 4;
  Buffer.from(requestBytes).copy(data, o); o += requestBytes.length;
  data.writeUInt32LE(relayInstructions.length, o); o += 4;
  relayInstructions.copy(data, o);
  return data;
}

/**
 * Build QuoteExecution instruction data for the router.
 * Layout: discriminator (1) + quoter_address (20) + quoter_cpi_data
 *
 * Quoter CPI data (passed to quoter):
 * - discriminator (8) - must be [2, 0, 0, 0, 0, 0, 0, 0]
 * - dst_chain (2)
 * - dst_addr (32)
 * - refund_addr (32)
 * - request_bytes_len (4) + request_bytes
 * - relay_instructions_len (4) + relay_instructions
 */
function buildQuoteExecutionData(
  quoterAddress: Uint8Array,
  dstChain: number,
  dstAddr: Uint8Array,
  refundAddr: Uint8Array,
  requestBytes: Uint8Array,
  relayInstructions: Uint8Array
): Buffer {
  // CPI data size: 8 + 2 + 32 + 32 + 4 + requestBytes.length + 4 + relayInstructions.length
  const cpiDataLen = 8 + 2 + 32 + 32 + 4 + requestBytes.length + 4 + relayInstructions.length;
  const data = Buffer.alloc(1 + 20 + cpiDataLen);
  let o = 0;

  // Router discriminator
  data.writeUInt8(IX_ROUTER_QUOTE_EXECUTION, o++);

  // Quoter address for registration lookup
  Buffer.from(quoterAddress).copy(data, o); o += 20;

  // Quoter CPI data - 8-byte discriminator [2, 0, 0, 0, 0, 0, 0, 0] for RequestQuote
  data.writeUInt8(2, o++); // instruction ID
  o += 7; // padding

  // Rest of quoter request data
  data.writeUInt16LE(dstChain, o); o += 2;
  Buffer.from(dstAddr).copy(data, o); o += 32;
  Buffer.from(refundAddr).copy(data, o); o += 32;
  data.writeUInt32LE(requestBytes.length, o); o += 4;
  Buffer.from(requestBytes).copy(data, o); o += requestBytes.length;
  data.writeUInt32LE(relayInstructions.length, o); o += 4;
  Buffer.from(relayInstructions).copy(data, o);
  return data;
}

function buildGasRelayInstruction(gasLimit: bigint, msgValue: bigint): Buffer {
  const data = Buffer.alloc(33);
  data.writeUInt8(1, 0);
  const gasLimitBuf = Buffer.alloc(16);
  gasLimitBuf.writeBigUInt64BE(gasLimit >> 64n, 0);
  gasLimitBuf.writeBigUInt64BE(gasLimit & 0xFFFFFFFFFFFFFFFFn, 8);
  gasLimitBuf.copy(data, 1);
  const msgValueBuf = Buffer.alloc(16);
  msgValueBuf.writeBigUInt64BE(msgValue >> 64n, 0);
  msgValueBuf.writeBigUInt64BE(msgValue & 0xFFFFFFFFFFFFFFFFn, 8);
  msgValueBuf.copy(data, 17);
  return data;
}

// Simulation helper

async function simulateInstruction(
  connection: Connection,
  wallet: Keypair,
  ix: TransactionInstruction
): Promise<{ computeUnits: number; returnData: Buffer }> {
  const { blockhash } = await connection.getLatestBlockhash();
  const msg = new TransactionMessage({
    payerKey: wallet.publicKey,
    recentBlockhash: blockhash,
    instructions: [ix],
  }).compileToV0Message();
  const tx = new VersionedTransaction(msg);

  const sim = await connection.simulateTransaction(tx, { sigVerify: false });

  if (sim.value.err) {
    throw new Error(`Simulation failed: ${JSON.stringify(sim.value.err)}\nLogs: ${sim.value.logs?.join("\n")}`);
  }
  if (!sim.value.returnData) {
    throw new Error("No return data");
  }

  return {
    computeUnits: sim.value.unitsConsumed ?? 0,
    returnData: Buffer.from(sim.value.returnData.data[0], "base64"),
  };
}

// Test context
let connection: Connection;
let wallet: Keypair;
let quoterConfigPda: PublicKey;
let quoterChainInfoPda: PublicKey;
let quoterQuoteBodyPda: PublicKey;
let routerConfigPda: PublicKey;
let quoterIdentity: QuoterIdentity;
let quoterRegistrationPda: PublicKey;

// Calculate expected quote value
const EXPECTED_QUOTE = calculateExpectedQuote(
  TEST_BASE_FEE,
  TEST_SRC_PRICE,
  TEST_DST_PRICE,
  TEST_DST_GAS_PRICE,
  TEST_GAS_PRICE_DECIMALS,
  TEST_NATIVE_DECIMALS,
  TEST_GAS_LIMIT,
  TEST_MSG_VALUE
);

describe("executor-quoter", () => {
  beforeAll(async () => {
    connection = new Connection("https://api.devnet.solana.com", "confirmed");
    wallet = loadWallet();

    [quoterConfigPda] = deriveQuoterConfigPda();
    [quoterChainInfoPda] = deriveQuoterChainInfoPda(CHAIN_ID_ETHEREUM);
    [quoterQuoteBodyPda] = deriveQuoterQuoteBodyPda(CHAIN_ID_ETHEREUM);
  });

  test("updates chain info for Ethereum", async () => {
    // Accounts: [payer, updater, chain_info, system_program]
    const ix = new TransactionInstruction({
      programId: QUOTER_PROGRAM_ID,
      keys: [
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
        { pubkey: quoterChainInfoPda, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data: buildUpdateChainInfoData(
        CHAIN_ID_ETHEREUM,
        1,
        TEST_GAS_PRICE_DECIMALS,
        TEST_NATIVE_DECIMALS,
      ),
    });

    const tx = new Transaction().add(ix);
    await sendAndConfirmTransaction(connection, tx, [wallet]);

    expect(await connection.getAccountInfo(quoterChainInfoPda)).not.toBeNull();
  });

  test("updates quote for Ethereum", async () => {
    // Accounts: [payer, updater, quote_body, system_program]
    const ix = new TransactionInstruction({
      programId: QUOTER_PROGRAM_ID,
      keys: [
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
        { pubkey: quoterQuoteBodyPda, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data: buildUpdateQuoteData(
        CHAIN_ID_ETHEREUM,
        TEST_DST_PRICE,
        TEST_SRC_PRICE,
        TEST_DST_GAS_PRICE,
        TEST_BASE_FEE
      ),
    });

    const tx = new Transaction().add(ix);
    await sendAndConfirmTransaction(connection, tx, [wallet]);

    expect(await connection.getAccountInfo(quoterQuoteBodyPda)).not.toBeNull();
  });

  test("returns correct quote via RequestQuote", async () => {
    const dstAddr = new Uint8Array(32).fill(0xAB);
    const refundAddr = new Uint8Array(32);
    wallet.publicKey.toBuffer().copy(Buffer.from(refundAddr));

    // Accounts: [_config, chain_info, quote_body]
    const ix = new TransactionInstruction({
      programId: QUOTER_PROGRAM_ID,
      keys: [
        { pubkey: quoterConfigPda, isSigner: false, isWritable: false },
        { pubkey: quoterChainInfoPda, isSigner: false, isWritable: false },
        { pubkey: quoterQuoteBodyPda, isSigner: false, isWritable: false },
      ],
      data: buildRequestQuoteData(
        CHAIN_ID_ETHEREUM,
        dstAddr,
        refundAddr,
        new Uint8Array(0),
        buildGasRelayInstruction(TEST_GAS_LIMIT, TEST_MSG_VALUE)
      ),
    });

    const { returnData } = await simulateInstruction(connection, wallet, ix);

    expect(returnData.length).toBe(8);
    const payment = returnData.readBigUInt64BE(0);
    expect(payment).toBe(EXPECTED_QUOTE);
  });

  test("msg_value increases the quote", async () => {
    const dstAddr = new Uint8Array(32).fill(0xAB);
    const refundAddr = new Uint8Array(32);
    wallet.publicKey.toBuffer().copy(Buffer.from(refundAddr));

    // Quote without msg_value
    const ixNoValue = new TransactionInstruction({
      programId: QUOTER_PROGRAM_ID,
      keys: [
        { pubkey: quoterConfigPda, isSigner: false, isWritable: false },
        { pubkey: quoterChainInfoPda, isSigner: false, isWritable: false },
        { pubkey: quoterQuoteBodyPda, isSigner: false, isWritable: false },
      ],
      data: buildRequestQuoteData(
        CHAIN_ID_ETHEREUM,
        dstAddr,
        refundAddr,
        new Uint8Array(0),
        buildGasRelayInstruction(TEST_GAS_LIMIT, 0n)
      ),
    });

    // Quote with msg_value
    const ixWithValue = new TransactionInstruction({
      programId: QUOTER_PROGRAM_ID,
      keys: [
        { pubkey: quoterConfigPda, isSigner: false, isWritable: false },
        { pubkey: quoterChainInfoPda, isSigner: false, isWritable: false },
        { pubkey: quoterQuoteBodyPda, isSigner: false, isWritable: false },
      ],
      data: buildRequestQuoteData(
        CHAIN_ID_ETHEREUM,
        dstAddr,
        refundAddr,
        new Uint8Array(0),
        buildGasRelayInstruction(TEST_GAS_LIMIT, TEST_MSG_VALUE)
      ),
    });

    const { returnData: noValueData } = await simulateInstruction(connection, wallet, ixNoValue);
    const { returnData: withValueData } = await simulateInstruction(connection, wallet, ixWithValue);

    const quoteNoValue = noValueData.readBigUInt64BE(0);
    const quoteWithValue = withValueData.readBigUInt64BE(0);

    // Calculate expected values
    const expectedNoValue = calculateExpectedQuote(
      TEST_BASE_FEE, TEST_SRC_PRICE, TEST_DST_PRICE, TEST_DST_GAS_PRICE,
      TEST_GAS_PRICE_DECIMALS, TEST_NATIVE_DECIMALS, TEST_GAS_LIMIT, 0n
    );
    const expectedWithValue = calculateExpectedQuote(
      TEST_BASE_FEE, TEST_SRC_PRICE, TEST_DST_PRICE, TEST_DST_GAS_PRICE,
      TEST_GAS_PRICE_DECIMALS, TEST_NATIVE_DECIMALS, TEST_GAS_LIMIT, TEST_MSG_VALUE
    );

    expect(quoteNoValue).toBe(expectedNoValue);
    expect(quoteWithValue).toBe(expectedWithValue);
    expect(quoteWithValue).toBeGreaterThan(quoteNoValue);

    // The difference should be 1 ETH * (ETH_price / SOL_price) in lamports
    // = 1 ETH * 15 = 15 SOL = 15_000_000_000 lamports
    const expectedDiff = 15_000_000_000n;
    expect(quoteWithValue - quoteNoValue).toBe(expectedDiff);
  });
});

describe("executor-quoter testnet chains", () => {
  beforeAll(async () => {
    connection = new Connection("https://api.devnet.solana.com", "confirmed");
    wallet = loadWallet();
    [quoterConfigPda] = deriveQuoterConfigPda();
  });

  for (const chain of TESTNET_CHAINS) {
    test(`updates chain info for ${chain.name} (${chain.chainId})`, async () => {
      const [chainInfoPda] = deriveQuoterChainInfoPda(chain.chainId);

      const ix = new TransactionInstruction({
        programId: QUOTER_PROGRAM_ID,
        keys: [
          { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
          { pubkey: chainInfoPda, isSigner: false, isWritable: true },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        data: buildUpdateChainInfoData(
          chain.chainId,
          1, // enabled
          chain.gasPriceDecimals,
          chain.nativeDecimals,
        ),
      });

      const tx = new Transaction().add(ix);
      await sendAndConfirmTransaction(connection, tx, [wallet]);

      const accountInfo = await connection.getAccountInfo(chainInfoPda);
      expect(accountInfo).not.toBeNull();
      console.log(`  Chain info PDA for ${chain.name}: ${chainInfoPda.toBase58()}`);
    });

    test(`updates quote for ${chain.name} (${chain.chainId})`, async () => {
      const [quoteBodyPda] = deriveQuoterQuoteBodyPda(chain.chainId);

      const ix = new TransactionInstruction({
        programId: QUOTER_PROGRAM_ID,
        keys: [
          { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
          { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
          { pubkey: quoteBodyPda, isSigner: false, isWritable: true },
          { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
        ],
        data: buildUpdateQuoteData(
          chain.chainId,
          chain.dstPrice,
          TEST_SRC_PRICE,
          chain.dstGasPrice,
          chain.baseFee
        ),
      });

      const tx = new Transaction().add(ix);
      await sendAndConfirmTransaction(connection, tx, [wallet]);

      const accountInfo = await connection.getAccountInfo(quoteBodyPda);
      expect(accountInfo).not.toBeNull();
      console.log(`  Quote body PDA for ${chain.name}: ${quoteBodyPda.toBase58()}`);
    });

    test(`returns correct quote for ${chain.name} (${chain.chainId})`, async () => {
      const [chainInfoPda] = deriveQuoterChainInfoPda(chain.chainId);
      const [quoteBodyPda] = deriveQuoterQuoteBodyPda(chain.chainId);

      const dstAddr = new Uint8Array(32).fill(0xAB);
      const refundAddr = new Uint8Array(32);
      wallet.publicKey.toBuffer().copy(Buffer.from(refundAddr));

      const ix = new TransactionInstruction({
        programId: QUOTER_PROGRAM_ID,
        keys: [
          { pubkey: quoterConfigPda, isSigner: false, isWritable: false },
          { pubkey: chainInfoPda, isSigner: false, isWritable: false },
          { pubkey: quoteBodyPda, isSigner: false, isWritable: false },
        ],
        data: buildRequestQuoteData(
          chain.chainId,
          dstAddr,
          refundAddr,
          new Uint8Array(0),
          buildGasRelayInstruction(TEST_GAS_LIMIT, TEST_MSG_VALUE)
        ),
      });

      const { returnData } = await simulateInstruction(connection, wallet, ix);

      expect(returnData.length).toBe(8);
      const payment = returnData.readBigUInt64BE(0);

      // Calculate expected quote
      const expectedQuote = calculateExpectedQuote(
        chain.baseFee,
        TEST_SRC_PRICE,
        chain.dstPrice,
        chain.dstGasPrice,
        chain.gasPriceDecimals,
        chain.nativeDecimals,
        TEST_GAS_LIMIT,
        TEST_MSG_VALUE
      );

      expect(payment).toBe(expectedQuote);
      console.log(`  Quote for ${chain.name}: ${payment} lamports (${Number(payment) / 1e9} SOL)`);
    });
  }
});

describe("executor-quoter-router", () => {
  beforeAll(async () => {
    connection = new Connection("https://api.devnet.solana.com", "confirmed");
    wallet = loadWallet();

    [quoterConfigPda] = deriveQuoterConfigPda();
    [quoterChainInfoPda] = deriveQuoterChainInfoPda(CHAIN_ID_ETHEREUM);
    [quoterQuoteBodyPda] = deriveQuoterQuoteBodyPda(CHAIN_ID_ETHEREUM);
    [routerConfigPda] = deriveRouterConfigPda();

    quoterIdentity = new QuoterIdentity();
    [quoterRegistrationPda] = deriveQuoterRegistrationPda(quoterIdentity.ethAddress);
  });

  test("registers quoter via governance", async () => {
    const expiryTime = BigInt(Math.floor(Date.now() / 1000) + 3600);

    // Accounts: [payer, sender, _config, quoter_registration, system_program]
    const ix = new TransactionInstruction({
      programId: ROUTER_PROGRAM_ID,
      keys: [
        { pubkey: wallet.publicKey, isSigner: true, isWritable: true },
        { pubkey: wallet.publicKey, isSigner: true, isWritable: false },
        { pubkey: routerConfigPda, isSigner: false, isWritable: false },
        { pubkey: quoterRegistrationPda, isSigner: false, isWritable: true },
        { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
      ],
      data: await buildUpdateQuoterContractData(
        quoterIdentity,
        QUOTER_PROGRAM_ID,
        wallet.publicKey,
        CHAIN_ID_SOLANA,
        expiryTime,
      ),
    });

    const tx = new Transaction().add(ix);
    await sendAndConfirmTransaction(connection, tx, [wallet]);

    expect(await connection.getAccountInfo(quoterRegistrationPda)).not.toBeNull();
  });

  test("returns correct quote via QuoteExecution CPI", async () => {
    const dstAddr = new Uint8Array(32).fill(0xAB);
    const refundAddr = new Uint8Array(32);
    wallet.publicKey.toBuffer().copy(Buffer.from(refundAddr));

    // Accounts: [quoter_registration, quoter_program, quoter_config, quoter_chain_info, quoter_quote_body]
    const ix = new TransactionInstruction({
      programId: ROUTER_PROGRAM_ID,
      keys: [
        { pubkey: quoterRegistrationPda, isSigner: false, isWritable: false },
        { pubkey: QUOTER_PROGRAM_ID, isSigner: false, isWritable: false },
        { pubkey: quoterConfigPda, isSigner: false, isWritable: false },
        { pubkey: quoterChainInfoPda, isSigner: false, isWritable: false },
        { pubkey: quoterQuoteBodyPda, isSigner: false, isWritable: false },
      ],
      data: buildQuoteExecutionData(
        quoterIdentity.ethAddress,
        CHAIN_ID_ETHEREUM,
        dstAddr,
        refundAddr,
        new Uint8Array(0),
        buildGasRelayInstruction(TEST_GAS_LIMIT, TEST_MSG_VALUE)
      ),
    });

    const { returnData } = await simulateInstruction(connection, wallet, ix);

    expect(returnData.length).toBe(8);
    const payment = returnData.readBigUInt64BE(0);
    expect(payment).toBe(EXPECTED_QUOTE);
  });
});

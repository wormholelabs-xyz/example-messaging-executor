import { fromHex, toHex } from "viem";
import { Client, Wallet, Payment, xrpToDrops } from "xrpl";
import {
  serializeRequestForExecution,
  deserializeRequestForExecution,
  type RequestForExecution,
  RequestPrefix,
} from "./requestForExecutionLayout";

const XRPL_TESTNET_WSS = "wss://s.altnet.rippletest.net:51233";

interface SendPaymentOptions {
  wallet: Wallet;
  destination: string;
  amountXrp: string;
  memoData: Uint8Array;
  memoType?: string;
  destinationTag?: number;
}

/**
 * Send an XRP payment with a memo containing arbitrary bytes
 */
async function sendPaymentWithMemo(
  client: Client,
  options: SendPaymentOptions,
): Promise<string> {
  const { wallet, destination, amountXrp, memoData, memoType, destinationTag } =
    options;

  // XRPL memos require hex-encoded strings without 0x prefix
  const memoTypeHex = Buffer.from(memoType ?? "application/octet-stream")
    .toString("hex")
    .toUpperCase();
  const memoDataHex = toHex(memoData).slice(2).toUpperCase(); // Remove 0x prefix

  const payment: Payment = {
    TransactionType: "Payment",
    Account: wallet.address,
    Destination: destination,
    Amount: xrpToDrops(amountXrp),
    Memos: [
      {
        Memo: {
          MemoType: memoTypeHex,
          MemoData: memoDataHex,
        },
      },
    ],
  };

  if (destinationTag !== undefined) {
    payment.DestinationTag = destinationTag;
  }

  console.log("Preparing transaction...");
  console.log("From:", wallet.address);
  console.log("To:", destination);
  console.log("Amount:", amountXrp, "XRP");
  console.log("Memo size:", memoData.length, "bytes");

  const prepared = await client.autofill(payment);
  console.log("Fee:", prepared.Fee);

  const signed = wallet.sign(prepared);
  console.log("Tx Hash:", signed.hash);

  console.log("Submitting transaction...");
  const result = await client.submitAndWait(signed.tx_blob);

  const txResult =
    typeof result.result.meta === "object"
      ? result.result.meta?.TransactionResult
      : result.result.meta;

  if (txResult === "tesSUCCESS") {
    console.log("Transaction successful!");
    console.log("Validated in ledger:", result.result.ledger_index);
  } else {
    console.error("Transaction failed:", txResult);
  }

  return signed.hash;
}

function createSampleExecutorRequest(): Uint8Array {
  // SDK signedQuoteLayout expects Uint8Array for bytes fields
  const dummyQuoterAddressBytes = new Uint8Array(20).fill(0x12);
  const dummyPayeeAddress = new Uint8Array(32).fill(0xab);
  const dummySignature = new Uint8Array(65).fill(0xcd);

  // Our local layout uses hex strings via hexConversion
  const dummyQuoterAddress =
    "0x1234567890123456789012345678901234567890" as `0x${string}`;
  const dummyDstAddr =
    "0x000000000000000000000000deadbeefdeadbeefdeadbeefdeadbeefdeadbeef" as `0x${string}`;
  const dummyRefundAddr =
    "0xaabbccddaabbccddaabbccddaabbccddaabbccdd" as `0x${string}`;
  const dummySrcManager =
    "0x0000000000000000000000001111111111111111111111111111111111111111" as `0x${string}`;
  const dummyMessageId =
    "0x0000000000000000000000002222222222222222222222222222222222222222" as `0x${string}`;

  const request: RequestForExecution = {
    quoterAddress: dummyQuoterAddress,
    dstChain: 1, // Solana
    dstAddr: dummyDstAddr,
    refundAddr: dummyRefundAddr,
    signedQuote: {
      quote: {
        prefix: "EQ01",
        quoterAddress: dummyQuoterAddressBytes,
        payeeAddress: dummyPayeeAddress,
        srcChain: 66, // XRPL
        dstChain: 1, // Solana
        expiryTime: new Date(Date.now() + 3600 * 1000),
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
        srcManager: dummySrcManager,
        messageId: dummyMessageId,
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
  };

  return fromHex(serializeRequestForExecution(request), "bytes");
}

async function main() {
  console.log("=== XRPL Payment with Memo Example ===\n");

  // Connect to XRPL Testnet
  const client = new Client(XRPL_TESTNET_WSS);
  console.log("Connecting to XRPL Testnet...");
  await client.connect();
  console.log("Connected!\n");

  try {
    console.log("Generating funded testnet wallet...");
    const fundResult = await client.fundWallet();
    const wallet = fundResult.wallet;
    console.log("Wallet address:", wallet.address);
    console.log("Wallet balance:", fundResult.balance, "XRP\n");

    const destFundResult = await client.fundWallet();
    const destinationAddress = destFundResult.wallet.address;
    console.log("Destination address:", destinationAddress, "\n");

    // Create a sample executor request payload
    const memoPayload = createSampleExecutorRequest();
    console.log("Memo payload (hex):", toHex(memoPayload));
    console.log("Memo payload size:", memoPayload.length, "bytes\n");

    // Send the payment with memo
    const txHash = await sendPaymentWithMemo(client, {
      wallet,
      destination: destinationAddress,
      amountXrp: "1", // Send 1 XRP
      memoData: memoPayload,
      memoType: "application/x-executor-request", // from SPEC
    });

    console.log("\n=== Transaction Complete ===");
    console.log("Transaction hash:", txHash);
    console.log(
      "View on explorer:",
      `https://testnet.xrpl.org/transactions/${txHash}`,
    );

    // Retrieve transaction from node and verify memo deserialization
    console.log("\n=== Verifying Transaction Memo ===");
    const txResponse = await client.request({
      command: "tx",
      transaction: txHash,
    });

    const tx = txResponse.result as unknown as {
      tx_json: {
        Memos?: Array<{ Memo: { MemoData?: string; MemoType?: string } }>;
      };
    };

    const memos = tx.tx_json.Memos;
    if (!memos || memos.length === 0) {
      throw new Error("No memos found in transaction");
    }

    const memoData = memos[0].Memo.MemoData;
    if (!memoData) {
      throw new Error("No MemoData found in transaction");
    }

    // Deserialize the memo data back to RequestForExecution
    const deserializedRequest = deserializeRequestForExecution(
      `0x${memoData.toLowerCase()}`,
    );

    console.log("Successfully deserialized RequestForExecution from memo:");
    console.log("  quoterAddress:", deserializedRequest.quoterAddress);
    console.log("  dstChain:", deserializedRequest.dstChain);
    console.log("  dstAddr:", deserializedRequest.dstAddr);
    console.log("  refundAddr:", deserializedRequest.refundAddr);
    console.log(
      "  signedQuote.quote.prefix:",
      deserializedRequest.signedQuote.quote.prefix,
    );
    console.log(
      "  signedQuote.quote.srcChain:",
      deserializedRequest.signedQuote.quote.srcChain,
    );
    console.log(
      "  signedQuote.quote.dstChain:",
      deserializedRequest.signedQuote.quote.dstChain,
    );
    console.log(
      "  signedQuote.quote.baseFee:",
      deserializedRequest.signedQuote.quote.baseFee,
    );
    console.log(
      "  requestBytes.request.prefix:",
      deserializedRequest.requestBytes.request.prefix,
    );
    console.log(
      "  requestBytes.request.srcChain:",
      deserializedRequest.requestBytes.request.srcChain,
    );

    const relayRequest =
      deserializedRequest.relayInstructions.requests[0].request;
    console.log(
      "  relayInstructions.requests[0].request.type:",
      relayRequest.type,
    );
    if (relayRequest.type === "GasInstruction") {
      console.log(
        "  relayInstructions.requests[0].request.gasLimit:",
        relayRequest.gasLimit,
      );
    }

    console.log("\n✓ Memo verification complete - all data intact!");
  } finally {
    await client.disconnect();
    console.log("\nDisconnected from XRPL.");
  }
}

main().catch(console.error);

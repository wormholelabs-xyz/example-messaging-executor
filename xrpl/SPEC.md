# XRPL Executor Specification

## Overview

This document describes how to compose a Request For Execution for NTT (Native Token Transfers) when sending an XRPL transaction.

## Executor Address

All execution requests MUST be sent as a payment to a predefined **Executor Address**. This address is monitored by the relay service for incoming transactions containing execution request memos.

[Executor Public Addresses](https://www.notion.so/wormholelabs/Executor-Addresses-Public-1f93029e88cb80df940eeb8867a01081?pvs=25)

## Transaction Format

To request execution, send an XRP payment with a memo containing the serialized `RequestForExecution` payload.

### Payment Structure

| Field             | Value                                                         |
| ----------------- | ------------------------------------------------------------- |
| TransactionType   | `Payment`                                                     |
| Destination       | Executor Address                                              |
| Amount            | Payment amount (covers relay fees)                            |
| Memos[0].MemoType | Hex-encoded string (e.g., `"application/x-executor-request"`) |
| Memos[0].MemoData | Hex-encoded `RequestForExecution` payload (no `0x` prefix)    |

## RequestForExecution Layout

```
[20]byte  quoterAddress      // EVM address of the quoter
uint16    dstChain           // Wormhole Chain ID of destination
[32]byte  dstAddr            // Universal address of destination contract
[20]byte  refundAddr         // EVM address for refunds
[2]byte   signedQuoteLen     // Length prefix for signedQuote
[var]byte signedQuote        // Signed quote payload (see below)
[2]byte   requestBytesLen    // Length prefix for requestBytes
[var]byte requestBytes       // Request payload (see below)
[2]byte   relayInstructionsLen // Length prefix for relayInstructions
[var]byte relayInstructions  // Relay instructions (see below)
```

## Signed Quote Layout (EQ01)

```
[4]byte   prefix = "EQ01"    // Quote version prefix (0x45513031)
[20]byte  quoterAddress      // Quoter's EVM address
[32]byte  payeeAddress       // Universal address of payee on source chain
uint16    srcChain           // Wormhole Chain ID of source (66 for XRPL)
uint16    dstChain           // Wormhole Chain ID of destination
uint64    expiryTime         // Unix timestamp when quote expires
uint64    baseFee            // Base fee in source chain currency
uint64    dstGasPrice        // Current gas price on destination chain
uint64    srcPrice           // USD price of source chain native currency (10^10)
uint64    dstPrice           // USD price of destination chain native currency (10^10)
[65]byte  signature          // Quoter's ECDSA signature
```

## NTT Request Layout (ERN1)

```
[4]byte   prefix = "ERN1"    // NTT v1 prefix (0x45524E31)
uint16    srcChain           // Wormhole Chain ID of source
[32]byte  srcManager         // Universal address of NTT manager on source
[32]byte  messageId          // Unique message identifier
```

## Relay Instructions Layout

Array of instructions, each prefixed with a type byte:

### Gas Instruction (type = 1)

```
uint8     type = 1           // Instruction type
uint128   gasLimit           // Gas limit for execution
uint128   msgValue           // Native value to send with execution
```

### Gas Drop-Off Instruction (type = 2)

```
uint8     type = 2           // Instruction type
uint128   dropOff            // Amount to drop off
[32]byte  recipient          // Universal address of drop-off recipient
```

## Example Flow

1. **Get Quote**: Request a signed quote from the quoter service for your source/destination chain pair
2. **Construct Request**: Build the `RequestForExecution` with:
   - The signed quote from step 1
   - NTT request bytes with the message details
   - Relay instructions specifying gas requirements
3. **Send Payment**: Submit an XRPL payment to the Executor Address with the serialized payload as memo
4. **Monitor**: The relay service monitors the Executor Address, parses incoming memos, and executes the relay on the destination chain

## Wormhole Chain IDs

| Chain    | ID  |
| -------- | --- |
| Solana   | 1   |
| Ethereum | 2   |
| XRPL     | 66  |

## Implementation Reference

See [sendPaymentWithMemo.ts](./examples/sendPaymentWithMemo.ts) for a working example of constructing and sending a `RequestForExecution` on XRPL.

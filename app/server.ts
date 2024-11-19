import "dotenv/config";
import express from "express";
import { privateKeyToAddress, recover } from "web3-eth-accounts";
import { sha3Raw } from "web3-utils";
import { createLogger, format, transports } from "winston";
import { BinaryReader, hexToUint8Array } from "./BinaryReader";
import { Handler } from "./handlers";
import { evmHandler } from "./handlers/evm";
import { makeSignedQuote } from "./signedQuote";

// Serialize BigInts as strings in responses
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore: Unreachable code error
BigInt.prototype.toJSON = function () {
  return this.toString();
};

const TEST_KEY =
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const QUOTER_PUBLIC_KEY = privateKeyToAddress(TEST_KEY);
const SUPPORTED_SRC_CHAINS = [2];
const SUPPORTED_DST_CHAINS = [5];
const CHAIN_TO_INFO: { [id: number]: { rpc: string; handler: Handler } } = {
  2: {
    rpc: "http://localhost:8545",
    handler: evmHandler,
  },
};

const logger = createLogger({
  level: process.env.LOG_LEVEL || "info",
  format: format.combine(
    format.simple(),
    format.errors({ stack: true }),
    format.timestamp({
      format: "YYYY-MM-DD HH:mm:ss.SSS ZZ",
    }),
    format.printf((info) => {
      // log format: [YYYY-MM-DD HH:mm:ss.SSS A ZZ] [level] [source] message
      const source = info.source || "main";
      return `[${info.timestamp}] [${info.level}] [${source}] ${info.message}`;
    })
  ),
  transports: [new transports.Console()],
});

const app = express();
const port = process.env.PORT || 3000;

app.get("/v0/quote/:srcChain/:dstChain", (req, res) => {
  let srcChain = 0;
  let dstChain = 0;
  try {
    srcChain = parseInt(req.params.srcChain);
    dstChain = parseInt(req.params.dstChain);
  } catch (e) {
    // will be unsupported
  }
  if (!SUPPORTED_SRC_CHAINS.includes(srcChain)) {
    res
      .status(400)
      .send(
        `Unsupported source chain: ${req.params.srcChain}, supported source chains: ${SUPPORTED_SRC_CHAINS}`
      );
    return;
  }
  if (!SUPPORTED_DST_CHAINS.includes(dstChain)) {
    res
      .status(400)
      .send(
        `Unsupported destination chain: ${req.params.dstChain}, supported destination chains: ${SUPPORTED_DST_CHAINS}`
      );
    return;
  }
  const expiryTime = new Date();
  expiryTime.setHours(expiryTime.getHours() + 1);
  res.json({
    signedQuote: makeSignedQuote(
      TEST_KEY,
      "000000000000000000000000ffffffffffffffffffffffffffffffffffffffff",
      parseInt(req.params.srcChain),
      parseInt(req.params.dstChain),
      BigInt(1), // TODO: make a config for base fee by destination chain
      BigInt(10000000000), // TODO: make a real quote here
      expiryTime
    ),
  });
});

app.get("/v0/status/:id", async (req, res) => {
  try {
    const reader = new BinaryReader(hexToUint8Array(req.params.id));
    const chainId = reader.readUint16();
    if (!SUPPORTED_SRC_CHAINS.includes(chainId)) {
      res
        .status(400)
        .send(
          `Unsupported source chain: ${chainId}, supported source chains: ${SUPPORTED_SRC_CHAINS}`
        );
      return;
    }
    const info = CHAIN_TO_INFO[chainId];
    if (!info) {
      res
        .status(400)
        .send(
          `Unsupported request chain: ${chainId}, supported request chains: ${Object.keys(CHAIN_TO_INFO)}`
        );
      return;
    }
    const requestForExecution = await info.handler.getRequest(info.rpc, reader);
    if (!requestForExecution) {
      res.sendStatus(404);
      return;
    }
    const signatureLenHex = 65 * 2;
    const quoteLen = requestForExecution.signedQuoteBytes.length;
    const body = requestForExecution.signedQuoteBytes.substring(
      0,
      quoteLen - signatureLenHex
    );
    const signature = `0x${requestForExecution.signedQuoteBytes.substring(
      quoteLen - signatureLenHex
    )}`;
    const recoveredPubKey = recover(sha3Raw(body), signature, true);
    if (recoveredPubKey !== QUOTER_PUBLIC_KEY) {
      res
        .status(400)
        .send(
          `Bad quote signature recovery. Expected: ${QUOTER_PUBLIC_KEY}, Received: ${recoveredPubKey}`
        );
      return;
    }
    // TODO: check if underpriced
    // TODO: background relay
    res.send({
      requestForExecution,
      status: "pending",
    });
    return;
  } catch (e) {
    res.status(400).send(`Bad request id`);
    return;
  }
});

app.listen(port, () => {
  logger.info(`Server is running at http://localhost:${port}`);
});

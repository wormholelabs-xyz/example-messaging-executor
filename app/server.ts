import axios from "axios";
import bs58 from "bs58";
import "dotenv/config";
import express from "express";
import { Chain, createPublicClient, createWalletClient, http } from "viem";
import { privateKeyToAccount, privateKeyToAddress } from "viem/accounts";
import { sepolia } from "viem/chains";
import { createLogger, format, Logger, transports } from "winston";
import { BinaryReader, hexToUint8Array } from "./BinaryReader";
import { MAX_U64 } from "./BinaryWriter";
import { Handler } from "./handlers";
import { evmHandler } from "./handlers/evm";
import { svmHandler } from "./handlers/svm";
import {
  decodeRelayInstructions,
  totalGasLimitAndMsgValue,
} from "./relayInstructions";
import {
  ModularMessageRequest,
  RequestForExecution,
  VAAv1Request,
} from "./requestForExecution";
import { SignedQuote } from "./signedQuote";

// Serialize BigInts as strings in responses
// eslint-disable-next-line @typescript-eslint/ban-ts-comment
// @ts-ignore: Unreachable code error
BigInt.prototype.toJSON = function () {
  return this.toString();
};

const envStringRequired = (name: string): string => {
  let s = process.env[name];
  if (!s) {
    throw new Error(`${name} is required!`);
  }
  return s;
};

const env0xStringRequired = (name: string): `0x${string}` => {
  // check hex regex?
  let s = envStringRequired(name);
  if (!s.startsWith("0x")) {
    throw new Error(`${name} must start with 0x!`);
  }
  return s as `0x${string}`;
};

const RELAY_SLEEP = 5000;
const ETH_KEY = env0xStringRequired("ETH_KEY");
const ETH_PUBLIC_KEY = privateKeyToAddress(ETH_KEY);
const SOL_PUBLIC_KEY = envStringRequired("SOL_PUBLIC_KEY");
const QUOTER_KEY = env0xStringRequired("QUOTER_KEY");
const QUOTER_PUBLIC_KEY = privateKeyToAddress(QUOTER_KEY);
const GUARDIAN_URL = envStringRequired("GUARDIAN_URL");
const SUPPORTED_SRC_CHAINS = [1, 10002];
const SUPPORTED_DST_CHAINS = [1, 10002];
const CHAIN_TO_INFO: {
  [id: number]: {
    rpc: string;
    handler: Handler;
    baseFee: bigint;
    coingeckoId: string;
    payeeAddress: string;
    gasPriceDecimals: number;
    nativeDecimals: number;
    executorAddress: string;
    evmChain?: Chain;
  };
} = {
  1: {
    rpc: "https://api.devnet.solana.com",
    handler: svmHandler,
    baseFee: 1000n,
    coingeckoId: "solana",
    payeeAddress: `0x${Buffer.from(bs58.decode(SOL_PUBLIC_KEY)).toString("hex")}`, // devnet key
    executorAddress: "Ax7mtQPbNPQmghd7C3BHrMdwwmkAXBDq7kNGfXNcc7dg",
    gasPriceDecimals: 9,
    nativeDecimals: 9,
  },
  10002: {
    rpc: "https://ethereum-sepolia-rpc.publicnode.com",
    handler: evmHandler,
    baseFee: 1000n,
    coingeckoId: "ethereum",
    payeeAddress: "0x000000000000000000000000" + ETH_PUBLIC_KEY.substring(2),
    gasPriceDecimals: 18,
    nativeDecimals: 18,
    executorAddress: "0xB67841A38bF16EB9999dC7B6015746506e20F0aA",
    evmChain: sepolia,
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
    }),
  ),
  transports: [new transports.Console()],
});

const app = express();
const port = process.env.PORT || 3000;

const priceCache: { [id: string]: { usd: number; expiry: Date } } = {};
async function updatePriceCache(ids: string[]) {
  const idsToQuery = [];
  const now = new Date();
  for (const id of ids) {
    if (!priceCache[id] || priceCache[id].expiry < now) {
      idsToQuery.push(id);
    }
  }
  if (idsToQuery.length) {
    try {
      const response = await axios.get(
        `https://api.coingecko.com/api/v3/simple/price?ids=${idsToQuery.join(",")}&vs_currencies=usd`,
      );
      const expiry = new Date(now);
      expiry.setMinutes(expiry.getMinutes() + 5);
      for (const id of idsToQuery) {
        if (response.data[id]) {
          priceCache[id] = { usd: response.data[id].usd, expiry };
        }
      }
    } catch (e) {}
  }
  console.log(priceCache);
}
async function getPrices(
  srcId: string,
  dstId: string,
): Promise<{ srcPrice: bigint; dstPrice: bigint }> {
  await updatePriceCache([srcId, dstId]);
  const cachedSrc = priceCache[srcId];
  const cachedDst = priceCache[dstId];
  const now = new Date();
  if (!cachedSrc || cachedSrc.expiry < now) {
    throw new Error(`expired source price`);
  }
  if (!cachedDst || cachedDst.expiry < now) {
    throw new Error(`expired destination price`);
  }
  // coingecko prices are a decimal number in USD
  // scale each price to the quote decimals
  const srcPrice = BigInt(
    priceCache[srcId].usd.toFixed(SignedQuote.decimals).replace(".", ""),
  );
  const dstPrice = BigInt(
    priceCache[dstId].usd.toFixed(SignedQuote.decimals).replace(".", ""),
  );
  return { srcPrice, dstPrice };
}

async function relayVAAv1(r: RequestForExecution, v: VAAv1Request) {
  const vaaId = `${v.chain}/${v.address.slice(2)}/${v.sequence.toString()}`;
  const bytes = (await axios.get(`${GUARDIAN_URL}/v1/signed_vaa/${vaaId}`)).data
    ?.vaaBytes;
  if (!bytes) {
    throw new Error(`unable to fetch VAA ${vaaId}`);
  }
  const dstInfo = CHAIN_TO_INFO[r.dstChain];
  const account = privateKeyToAccount(ETH_KEY);
  const publicClient = createPublicClient({
    chain: dstInfo.evmChain,
    transport: http(dstInfo.rpc),
  });
  const relayInstructions = decodeRelayInstructions(r.relayInstructionsBytes);
  const { gasLimit, msgValue } = totalGasLimitAndMsgValue(relayInstructions);
  const { request } = await publicClient.simulateContract({
    account,
    address: `0x${r.dstAddr.substring(26)}`,
    gas: gasLimit,
    value: msgValue,
    abi: [
      {
        type: "function",
        name: "receiveMessage",
        inputs: [{ type: "bytes" }],
        outputs: [],
      },
    ],
    functionName: "receiveMessage",
    args: [`0x${Buffer.from(bytes, "base64").toString("hex")}`],
  });
  const client = createWalletClient({
    account,
    chain: dstInfo.evmChain,
    transport: http(dstInfo.rpc),
  });
  return await client.writeContract(request);
}
async function relayMM(r: RequestForExecution, m: ModularMessageRequest) {
  const dstInfo = CHAIN_TO_INFO[r.dstChain];
  const account = privateKeyToAccount(ETH_KEY);
  const publicClient = createPublicClient({
    chain: dstInfo.evmChain,
    transport: http(dstInfo.rpc),
  });
  const relayInstructions = decodeRelayInstructions(r.relayInstructionsBytes);
  const { gasLimit, msgValue } = totalGasLimitAndMsgValue(relayInstructions);
  // TODO: call `isReady` first before attempting relay
  const { request } = await publicClient.simulateContract({
    account,
    address: `0x${r.dstAddr.substring(26)}`,
    gas: gasLimit,
    value: msgValue,
    abi: [
      {
        type: "function",
        name: "executeMsg",
        inputs: [
          { type: "uint16" },
          { type: "bytes32" },
          { type: "uint64" },
          { type: "bytes" },
        ],
        outputs: [],
      },
    ],
    functionName: "executeMsg",
    args: [m.chain, m.address, m.sequence, m.payload],
  });
  const walletClient = createWalletClient({
    account,
    chain: dstInfo.evmChain,
    transport: http(dstInfo.rpc),
  });
  return await walletClient.writeContract(request);
}
const relays: {
  [id: string]: {
    status: string;
    requestForExecution: RequestForExecution;
    txs: string[];
    instruction?: VAAv1Request | ModularMessageRequest;
  };
} = {};
const pendingRelays: string[] = [];
async function relayNext(logger: Logger) {
  const id = pendingRelays.shift();
  if (id === undefined) {
    return;
  }
  const r = relays[id];
  logger.info(JSON.stringify(r));
  if (r.instruction) {
    try {
      if (r.instruction instanceof VAAv1Request) {
        const tx = await relayVAAv1(r.requestForExecution, r.instruction);
        relays[id].status = "submitted";
        relays[id].txs.push(tx);
      } else if (r.instruction instanceof ModularMessageRequest) {
        const tx = await relayMM(r.requestForExecution, r.instruction);
        relays[id].status = "submitted";
        relays[id].txs.push(tx);
      } else {
        relays[id].status = "unsupported";
      }
    } catch (e: any) {
      logger.error(e);
      // TODO: handle this better
      if (e?.message?.includes("reverted")) {
        relays[id].status = "failed";
      } else {
        pendingRelays.push(id);
      }
    }
  } else {
    relays[id].status = "unsupported";
  }
}
async function sleep(timeout: number) {
  return new Promise((resolve) => setTimeout(resolve, timeout));
}
async function runWithRetry(
  fn: (logger: Logger) => Promise<void>,
  timeout: number,
  logger: Logger,
) {
  let retry = 0;
  while (true) {
    try {
      await fn(logger);
      retry = 0;
      await sleep(timeout);
    } catch (e) {
      retry++;
      logger.error(e);
      const expoBacko = timeout * 2 ** retry;
      logger.warn(`backing off for ${expoBacko}ms`);
      await sleep(expoBacko);
    }
  }
}
runWithRetry(relayNext, RELAY_SLEEP, logger.child({ source: "relay" }));

app.get("/v0/quote/:srcChain/:dstChain", async (req, res) => {
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
        `Unsupported source chain: ${req.params.srcChain}, supported source chains: ${SUPPORTED_SRC_CHAINS}`,
      );
    return;
  }
  if (!SUPPORTED_DST_CHAINS.includes(dstChain)) {
    res
      .status(400)
      .send(
        `Unsupported destination chain: ${req.params.dstChain}, supported destination chains: ${SUPPORTED_DST_CHAINS}`,
      );
    return;
  }
  const srcInfo = CHAIN_TO_INFO[srcChain];
  const dstInfo = CHAIN_TO_INFO[dstChain];
  if (!srcInfo) {
    res
      .status(400)
      .send(
        `Unsupported source chain: ${srcChain}, supported request chains: ${Object.keys(CHAIN_TO_INFO)}`,
      );
    return;
  }
  if (!dstInfo) {
    res
      .status(400)
      .send(
        `Unsupported destination chain: ${dstChain}, supported request chains: ${Object.keys(CHAIN_TO_INFO)}`,
      );
    return;
  }
  try {
    const dstGasPrice = await dstInfo.handler.getGasPrice(dstInfo.rpc);
    const { srcPrice, dstPrice } = await getPrices(
      srcInfo.coingeckoId,
      dstInfo.coingeckoId,
    );
    if (srcPrice === 0n || srcPrice > MAX_U64) {
      res.status(400).send(`source price out of range`);
      return;
    }
    if (dstPrice === 0n || dstPrice > MAX_U64) {
      res.status(400).send(`destination price out of range`);
      return;
    }
    const expiryTime = new Date();
    expiryTime.setHours(expiryTime.getHours() + 1);
    res.json({
      signedQuote: await new SignedQuote(
        QUOTER_PUBLIC_KEY,
        srcInfo.payeeAddress,
        parseInt(req.params.srcChain),
        parseInt(req.params.dstChain),
        expiryTime,
        dstInfo.baseFee,
        dstGasPrice,
        srcPrice,
        dstPrice,
      ).sign(QUOTER_KEY),
    });
  } catch (e: any) {
    console.log(e);
    res.status(400).send(e?.message || "Unable to generate quote");
  }
});

app.get("/v0/estimate/:quote/:relayInstructions", async (req, res) => {
  try {
    const quote = SignedQuote.from(req.params.quote);
    await quote.verify([QUOTER_PUBLIC_KEY.toLowerCase()]);
    const srcInfo = CHAIN_TO_INFO[quote.srcChain];
    if (!srcInfo) {
      res
        .status(400)
        .send(
          `Unsupported request chain: ${quote.srcChain}, supported request chains: ${Object.keys(CHAIN_TO_INFO)}`,
        );
      return;
    }
    const dstInfo = CHAIN_TO_INFO[quote.dstChain];
    if (!srcInfo) {
      res
        .status(400)
        .send(
          `Unsupported destination chain: ${quote.dstChain}, supported request chains: ${Object.keys(CHAIN_TO_INFO)}`,
        );
      return;
    }
    const relayInstructions = decodeRelayInstructions(
      req.params.relayInstructions,
    );
    const { gasLimit, msgValue } = totalGasLimitAndMsgValue(relayInstructions);
    const estimate = quote.estimate(
      gasLimit,
      msgValue,
      dstInfo.gasPriceDecimals,
      srcInfo.nativeDecimals,
      dstInfo.nativeDecimals,
    );
    res.send({ quote, estimate });
  } catch (e: any) {
    res.status(400).send(e?.message || "Bad request");
  }
});

app.get("/v0/request/VAAv1/:chain/:emitter/:sequence", (req, res) => {
  try {
    res.send({
      bytes: new VAAv1Request(
        parseInt(req.params.chain),
        req.params.emitter,
        BigInt(req.params.sequence),
      ).serialize(),
    });
  } catch (e) {
    res.sendStatus(400);
  }
});

app.get("/v0/request/MM/:chain/:emitter/:sequence/:payload", (req, res) => {
  try {
    res.send({
      bytes: new ModularMessageRequest(
        parseInt(req.params.chain),
        req.params.emitter,
        BigInt(req.params.sequence),
        req.params.payload,
      ).serialize(),
    });
  } catch (e) {
    res.sendStatus(400);
  }
});

// TODO: status an entire transaction
app.get("/v0/status/:id", async (req, res) => {
  // TODO: normalize id
  if (relays[req.params.id]) {
    res.send(relays[req.params.id]);
    return;
  }
  try {
    const reader = new BinaryReader(hexToUint8Array(req.params.id));
    const chainId = reader.readUint16();
    if (!SUPPORTED_SRC_CHAINS.includes(chainId)) {
      res
        .status(400)
        .send(
          `Unsupported source chain: ${chainId}, supported source chains: ${SUPPORTED_SRC_CHAINS}`,
        );
      return;
    }
    const srcInfo = CHAIN_TO_INFO[chainId];
    if (!srcInfo) {
      res
        .status(400)
        .send(
          `Unsupported request chain: ${chainId}, supported request chains: ${Object.keys(CHAIN_TO_INFO)}`,
        );
      return;
    }
    const requestForExecution = await srcInfo.handler.getRequest(
      srcInfo.rpc,
      srcInfo.executorAddress,
      reader,
    );
    if (!requestForExecution) {
      res.sendStatus(404);
      return;
    }
    const dstInfo = CHAIN_TO_INFO[requestForExecution.dstChain];
    if (!dstInfo) {
      res
        .status(400)
        .send(
          `Unsupported destination chain: ${requestForExecution.dstChain}, supported request chains: ${Object.keys(CHAIN_TO_INFO)}`,
        );
      return;
    }
    const quote = SignedQuote.from(requestForExecution.signedQuoteBytes);
    try {
      await quote.verify([QUOTER_PUBLIC_KEY.toLowerCase()]);
    } catch (e: any) {
      res.status(400).send(e?.message || "Bad quote");
      return;
    }
    const relayInstructions = decodeRelayInstructions(
      requestForExecution.relayInstructionsBytes,
    );
    const { gasLimit, msgValue } = totalGasLimitAndMsgValue(relayInstructions);
    const estimate = quote.estimate(
      gasLimit,
      msgValue,
      dstInfo.gasPriceDecimals,
      srcInfo.nativeDecimals,
      dstInfo.nativeDecimals,
    );
    let instruction: VAAv1Request | ModularMessageRequest | undefined;
    try {
      instruction = VAAv1Request.from(requestForExecution.requestBytes);
    } catch (e) {
      try {
        instruction = ModularMessageRequest.from(
          requestForExecution.requestBytes,
        );
      } catch (e) {}
    }
    const status =
      requestForExecution.amtPaid < estimate
        ? "underpaid"
        : instruction
          ? "pending"
          : "unsupported";
    if (status === "pending") {
      // TODO: standardize id on RFE and use here
      if (!relays[req.params.id]) {
        relays[req.params.id] = {
          status,
          requestForExecution,
          instruction,
          txs: [],
        };
        pendingRelays.push(req.params.id);
      }
    }
    res.send({
      requestForExecution,
      instruction,
      quote,
      estimate,
      status,
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

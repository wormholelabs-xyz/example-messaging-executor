import { bs58 } from "@coral-xyz/anchor/dist/cjs/utils/bytes";
import { PublicKey } from "@solana/web3.js";
import { expect, test } from "bun:test";
import { fromBytes } from "viem";
import { svmNttHandler } from ".";

const solanaChainInfo = {
  chainId: 1,
  rpc: "https://api.devnet.solana.com",
  baseFee: 1000n,
  coingeckoId: "solana",
  payeeAddress: "",
  executorAddress: "Ax7mtQPbNPQmghd7C3BHrMdwwmkAXBDq7kNGfXNcc7dg",
  gasPriceDecimals: 9 + 6, // microlamports
  nativeDecimals: 9,
};

test("getTransceivers", async () => {
  const programIdHex = fromBytes(
    new PublicKey("nTTKNtbdt6WkS3igaGip9tezrBMzWHs4xeeqErDpUe4").toBytes(),
    "hex",
  );
  expect(
    await svmNttHandler.getEnabledTransceivers(solanaChainInfo, programIdHex),
  ).toEqual([{ address: programIdHex, type: "wormhole" }]);
});

test("getTransactionMessages", async () => {
  expect(
    await svmNttHandler.getTransferMessages(
      solanaChainInfo,
      fromBytes(
        bs58.decode(
          "44EAFCgtLZYkbw2yy8RJ2XuZaAvwMnNQPJuVYjcC1xvUwVWrzBvX3U4aGTTsNkGdAuEZEGh69f76Qt8V9u8kovLB",
        ),
        "hex",
      ),
      fromBytes(
        new PublicKey("nTTKNtbdt6WkS3igaGip9tezrBMzWHs4xeeqErDpUe4").toBytes(),
        "hex",
      ),
      fromBytes(
        bs58.decode("D6JAgRaRPNzGHoWQcBRZCxaAi6SvT4qwCgPeaeoZqKjc"),
        "hex",
      ),
    ),
  ).toEqual([
    {
      address:
        "0x6187d46e2cd6befe5b4377c312371ef4641559586bdea33cf38cdd72b8c27141",
      id: "1/6187d46e2cd6befe5b4377c312371ef4641559586bdea33cf38cdd72b8c27141/2",
      type: "wormhole",
    },
  ]);
});

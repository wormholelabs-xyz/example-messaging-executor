import { expect, test } from "bun:test";
import { SignedQuote } from "./signedQuote";

// anvil acct 0
const mockSigner =
  "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
const mockQuoter = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266";
const mockPayee =
  "0x4567456745674567456745674567456745674567456745674567456745674567";
const mockSignature =
  "0x89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89ab89";

test("constructor", () => {
  const now = new Date();
  const sq = new SignedQuote(
    mockQuoter,
    mockPayee,
    1,
    2,
    now,
    100n,
    200n,
    300n,
    400n,
    mockSignature,
  );
  expect(sq.quoterAddress).toBe(mockQuoter);
  expect(sq.payeeAddress).toBe(mockPayee);
  expect(sq.srcChain).toBe(1);
  expect(sq.dstChain).toBe(2);
  expect(sq.expiryTime).toBe(now);
  expect(sq.baseFee).toBe(100n);
  expect(sq.dstGasPrice).toBe(200n);
  expect(sq.srcPrice).toBe(300n);
  expect(sq.dstPrice).toBe(400n);
  expect(sq.signature).toBe(mockSignature);
});

test("symmetric serialize and from", async () => {
  const sq = new SignedQuote(
    mockQuoter,
    mockPayee,
    1,
    2,
    new Date(),
    100n,
    200n,
    300n,
    400n,
  );
  const serialized = await sq.sign(mockSigner);
  const from = SignedQuote.from(serialized);
  const reserialized = from.serializeBody() + from.signature?.substring(2);
  expect(serialized).toBe(reserialized);
});

test("verify", async () => {
  const sq = new SignedQuote(
    mockQuoter,
    mockPayee,
    1,
    2,
    new Date(),
    100n,
    200n,
    300n,
    400n,
  );
  await sq.sign(mockSigner);
  expect(sq.verify([mockQuoter])).resolves;
});

test("estimate", async () => {
  const sq = new SignedQuote(
    mockQuoter,
    mockPayee,
    1,
    2,
    new Date(),
    100n,
    200n,
    300n,
    400n,
  );
  expect(sq.estimate(1000n, 0n, 18, 18, 18)).toBe(266666n);
});

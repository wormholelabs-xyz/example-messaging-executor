// Converts Shank-generated Anchor IDLs to Codama IDL format, then enriches
// them with instruction arguments, PDA definitions, and error codes that
// Shank cannot infer from the Pinocchio source.
//
// Usage: bun scripts/generate-codama-idl.mjs

import { rootNodeFromAnchor } from "@codama/nodes-from-anchor";
import {
  addPdasVisitor,
  bottomUpTransformerVisitor,
  bytesTypeNode,
  constantDiscriminatorNode,
  constantPdaSeedNodeFromString,
  constantValueNodeFromBytes,
  createFromRoot,
  errorNode,
  fixedSizeTypeNode,
  instructionArgumentNode,
  numberTypeNode,
  numberValueNode,
  sizePrefixTypeNode,
  variablePdaSeedNode,
} from "codama";
import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");
const anchorIdlDir = path.join(root, "idl");
const codamaIdlDir = path.join(root, "idl", "codama");

mkdirSync(codamaIdlDir, { recursive: true });

// -- Helpers ------------------------------------------------------------------

function numArg(name, format) {
  return instructionArgumentNode({ name, type: numberTypeNode(format) });
}

function bytesArg(name, size) {
  return instructionArgumentNode({
    name,
    type: fixedSizeTypeNode(bytesTypeNode(), size),
  });
}

function prefixedBytesArg(name) {
  return instructionArgumentNode({
    name,
    type: sizePrefixTypeNode(bytesTypeNode(), numberTypeNode("u32")),
  });
}

function discArg(value) {
  return instructionArgumentNode({
    name: "discriminator",
    type: numberTypeNode("u8"),
    defaultValue: numberValueNode(value),
    defaultValueStrategy: "omitted",
  });
}

// -- Instruction argument definitions -----------------------------------------

// UpdateChainInfoData { chain_id: u16, enabled: u8, gas_price_decimals: u8,
//                       native_decimals: u8, _padding: u8 }
const updateChainInfoArgs = [
  discArg(0),
  numArg("chainId", "u16"),
  numArg("enabled", "u8"),
  numArg("gasPriceDecimals", "u8"),
  numArg("nativeDecimals", "u8"),
  numArg("padding", "u8"),
];

// UpdateQuoteData { chain_id: u16, _padding: [u8; 6], dst_price: u64,
//                   src_price: u64, dst_gas_price: u64, base_fee: u64 }
const updateQuoteArgs = [
  discArg(1),
  numArg("chainId", "u16"),
  bytesArg("padding", 6),
  numArg("dstPrice", "u64"),
  numArg("srcPrice", "u64"),
  numArg("dstGasPrice", "u64"),
  numArg("baseFee", "u64"),
];

// Shared fields for CPI quote instructions (after 8-byte constant discriminator):
// dst_chain: u16, dst_addr: [u8; 32], refund_addr: [u8; 32],
// request_bytes: prefixed(u32), relay_instructions: prefixed(u32)
const cpiQuoteFields = [
  numArg("dstChain", "u16"),
  bytesArg("dstAddr", 32),
  bytesArg("refundAddr", 32),
  prefixedBytesArg("requestBytes"),
  prefixedBytesArg("relayInstructions"),
];

// GovernanceMessage (163 bytes, opaque)
const updateQuoterContractArgs = [
  discArg(0),
  bytesArg("governanceMessage", 163),
];

// quoteExecution: disc=1, quoter_address: bytes[20], cpi_data: bytes(rest)
const quoteExecutionArgs = [
  discArg(1),
  bytesArg("quoterAddress", 20),
  instructionArgumentNode({ name: "cpiData", type: bytesTypeNode() }),
];

// requestExecution: disc=2, amount: u64, quoter_address: bytes[20],
//                   cpi_data: bytes(rest)
const requestExecutionArgs = [
  discArg(2),
  numArg("amount", "u64"),
  bytesArg("quoterAddress", 20),
  instructionArgumentNode({ name: "cpiData", type: bytesTypeNode() }),
];

// -- Error definitions --------------------------------------------------------

const EXECUTOR_QUOTER_ERRORS = [
  ["invalidUpdater", 0x1000, "Invalid updater authority"],
  ["chainDisabled", 0x1001, "Chain is disabled"],
  ["unsupportedInstruction", 0x1002, "Unsupported instruction"],
  ["moreThanOneDropOff", 0x1003, "More than one drop-off"],
  ["mathOverflow", 0x1004, "Math overflow"],
  ["invalidRelayInstructions", 0x1005, "Invalid relay instructions"],
  ["invalidPda", 0x1006, "Invalid PDA"],
  ["alreadyInitialized", 0x1007, "Already initialized"],
  ["notInitialized", 0x1008, "Not initialized"],
  ["invalidOwner", 0x1009, "Invalid owner"],
  ["invalidInstructionData", 0x100a, "Invalid instruction data"],
  ["invalidDiscriminator", 0x100b, "Invalid discriminator"],
  ["chainIdMismatch", 0x100c, "Chain ID mismatch"],
];

const EXECUTOR_QUOTER_ROUTER_ERRORS = [
  ["invalidOwner", 0, "Invalid owner"],
  ["invalidDiscriminator", 1, "Invalid discriminator"],
  ["invalidGovernancePrefix", 2, "Invalid governance prefix"],
  ["chainIdMismatch", 3, "Chain ID mismatch"],
  ["invalidSender", 4, "Invalid sender"],
  ["governanceExpired", 5, "Governance message expired"],
  ["invalidSignature", 6, "Invalid signature"],
  ["notAnEvmAddress", 7, "Not an EVM address"],
  ["quoterNotRegistered", 8, "Quoter not registered"],
  ["underpaid", 9, "Underpaid"],
  ["refundFailed", 10, "Refund failed"],
  ["invalidInstructionData", 11, "Invalid instruction data"],
  ["cpiFailed", 12, "CPI failed"],
  ["invalidReturnData", 13, "Invalid return data"],
  ["mathOverflow", 14, "Math overflow"],
  ["invalidAccountData", 15, "Invalid account data"],
];

// -- Enrichment visitors ------------------------------------------------------

function enrichExecutorQuoter(codama) {
  // 1. Instruction arguments and discriminator fixes.
  codama.update(
    bottomUpTransformerVisitor([
      (node) => {
        if (node.kind !== "instructionNode") return node;
        switch (node.name) {
          case "updateChainInfo":
            return { ...node, arguments: updateChainInfoArgs };
          case "updateQuote":
            return { ...node, arguments: updateQuoteArgs };
          case "requestQuote":
            return {
              ...node,
              arguments: cpiQuoteFields,
              discriminators: [
                constantDiscriminatorNode(
                  constantValueNodeFromBytes("base16", "0200000000000000"),
                ),
              ],
            };
          case "requestExecutionQuote":
            return {
              ...node,
              arguments: [...cpiQuoteFields],
              discriminators: [
                constantDiscriminatorNode(
                  constantValueNodeFromBytes("base16", "0300000000000000"),
                ),
              ],
            };
          default:
            return node;
        }
      },
    ]),
  );

  // 2. PDA definitions.
  codama.update(
    addPdasVisitor({
      executorQuoter: [
        {
          name: "quoteBodyPda",
          seeds: [
            constantPdaSeedNodeFromString("utf8", "quote"),
            variablePdaSeedNode("chainId", numberTypeNode("u16")),
          ],
        },
        {
          name: "chainInfoPda",
          seeds: [
            constantPdaSeedNodeFromString("utf8", "chain_info"),
            variablePdaSeedNode("chainId", numberTypeNode("u16")),
          ],
        },
      ],
    }),
  );

  // 3. Convert error enum in definedTypes to proper errorNode entries.
  codama.update(
    bottomUpTransformerVisitor([
      (node) => {
        if (node.kind !== "programNode") return node;
        return {
          ...node,
          errors: EXECUTOR_QUOTER_ERRORS.map(([name, code, message]) =>
            errorNode({ name, code, message }),
          ),
          definedTypes: node.definedTypes.filter(
            (t) => t.name !== "executorQuoterError",
          ),
        };
      },
    ]),
  );
}

function enrichExecutorQuoterRouter(codama) {
  // 1. Instruction arguments.
  codama.update(
    bottomUpTransformerVisitor([
      (node) => {
        if (node.kind !== "instructionNode") return node;
        switch (node.name) {
          case "updateQuoterContract":
            return { ...node, arguments: updateQuoterContractArgs };
          case "quoteExecution":
            return { ...node, arguments: quoteExecutionArgs };
          case "requestExecution":
            return { ...node, arguments: requestExecutionArgs };
          default:
            return node;
        }
      },
    ]),
  );

  // 2. PDA definitions.
  codama.update(
    addPdasVisitor({
      executorQuoterRouter: [
        {
          name: "quoterRegistrationPda",
          seeds: [
            constantPdaSeedNodeFromString("utf8", "quoter_registration"),
            variablePdaSeedNode(
              "quoterAddress",
              fixedSizeTypeNode(bytesTypeNode(), 20),
            ),
          ],
        },
      ],
    }),
  );

  // 3. Convert error enum in definedTypes to proper errorNode entries.
  codama.update(
    bottomUpTransformerVisitor([
      (node) => {
        if (node.kind !== "programNode") return node;
        return {
          ...node,
          errors: EXECUTOR_QUOTER_ROUTER_ERRORS.map(([name, code, message]) =>
            errorNode({ name, code, message }),
          ),
          definedTypes: node.definedTypes.filter(
            (t) => t.name !== "executorQuoterRouterError",
          ),
        };
      },
    ]),
  );
}

// -- Main ---------------------------------------------------------------------

const programs = ["executor_quoter", "executor_quoter_router"];

for (const program of programs) {
  const anchorIdlPath = path.join(anchorIdlDir, `${program}.json`);
  const anchorIdl = JSON.parse(readFileSync(anchorIdlPath, "utf-8"));

  const codama = createFromRoot(rootNodeFromAnchor(anchorIdl));

  if (program === "executor_quoter") {
    enrichExecutorQuoter(codama);
  } else if (program === "executor_quoter_router") {
    enrichExecutorQuoterRouter(codama);
  }

  const json = codama.getJson();
  const formatted = JSON.stringify(JSON.parse(json), null, 2);

  const outputPath = path.join(codamaIdlDir, `${program}.json`);
  writeFileSync(outputPath, formatted);
  console.log(`Generated: ${outputPath}`);
}

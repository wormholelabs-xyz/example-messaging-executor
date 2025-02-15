import { deserializePayload } from "@wormhole-foundation/sdk-definitions";
import "@wormhole-foundation/sdk-definitions-ntt"; // register definition for parsing
import {
  createPublicClient,
  fromBytes,
  getContract,
  http,
  isAddressEqual,
  padHex,
  parseEventLogs,
} from "viem";
import { NttHandler } from "..";

export const evmNttHandler: NttHandler = {
  async getEnabledTransceivers(chainInfo, address, blockNumber) {
    const client = createPublicClient({
      chain: chainInfo.evmChain,
      transport: http(chainInfo.rpc),
      batch: {
        multicall: true,
      },
    });
    // get enabled transceivers
    const transceiverAddresses = await getContract({
      address,
      abi: [
        {
          inputs: [],
          name: "getTransceivers",
          outputs: [
            { internalType: "address[]", name: "result", type: "address[]" },
          ],
          stateMutability: "pure",
          type: "function",
        },
      ],
      client,
    }).read.getTransceivers({ blockNumber });
    // fetch each transceiver's type
    const getTransceiverType = async (address: `0x${string}`) => {
      // getTransceiverType did not exist prior to 1.1.0, so assume `wormhole` if this reverts
      try {
        return await getContract({
          address,
          abi: [
            {
              type: "function",
              name: "getTransceiverType",
              inputs: [],
              outputs: [{ name: "", type: "string", internalType: "string" }],
              stateMutability: "view",
            },
          ],
          client,
        }).read.getTransceiverType({ blockNumber });
      } catch (e) {
        return "wormhole";
      }
    };
    const transceiverTypes = await Promise.all(
      transceiverAddresses.map(getTransceiverType),
    );
    return transceiverAddresses.map((address, idx) => ({
      address,
      type: transceiverTypes[idx],
    }));
  },

  async getTransferMessages(chainInfo, hash, address, messageId) {
    const client = createPublicClient({
      chain: chainInfo.evmChain,
      transport: http(chainInfo.rpc),
      batch: {
        multicall: true,
      },
    });
    const transaction = await client.getTransactionReceipt({ hash });
    const transceivers = await this.getEnabledTransceivers(
      chainInfo,
      address,
      // may result in `missing trie node` if not an archive node
      // transaction.blockNumber,
    );
    // TODO: move to a function and Promise.all to batch
    const supportedMessages = [];
    for (const transceiver of transceivers) {
      if (transceiver.type === "wormhole") {
        try {
          const wormhole = await getContract({
            address: transceiver.address,
            abi: [
              {
                type: "function",
                name: "wormhole",
                inputs: [],
                outputs: [
                  {
                    name: "",
                    type: "address",
                    internalType: "contract IWormhole",
                  },
                ],
                stateMutability: "view",
              },
            ],
            client,
          }).read.wormhole({
            // may result in `missing trie node` if not an archive node
            // blockNumber: transaction.blockNumber,
          });
          const topics = parseEventLogs({
            eventName: "LogMessagePublished",
            abi: [
              {
                type: "event",
                name: "LogMessagePublished",
                inputs: [
                  {
                    name: "sender",
                    type: "address",
                    indexed: true,
                    internalType: "address",
                  },
                  {
                    name: "sequence",
                    type: "uint64",
                    indexed: false,
                    internalType: "uint64",
                  },
                  {
                    name: "nonce",
                    type: "uint32",
                    indexed: false,
                    internalType: "uint32",
                  },
                  {
                    name: "payload",
                    type: "bytes",
                    indexed: false,
                    internalType: "bytes",
                  },
                  {
                    name: "consistencyLevel",
                    type: "uint8",
                    indexed: false,
                    internalType: "uint8",
                  },
                ],
                anonymous: false,
              },
            ],
            logs: transaction.logs,
          });
          for (const topic of topics) {
            if (
              topic.removed === false &&
              isAddressEqual(topic.address, wormhole) &&
              isAddressEqual((topic.args as any).sender, transceiver.address)
            ) {
              const payload = deserializePayload(
                "Ntt:WormholeTransfer",
                (topic.args as any).payload,
              );
              const hexId = fromBytes(payload.nttManagerPayload.id, "hex");
              if (messageId === hexId) {
                supportedMessages.push({
                  ...transceiver,
                  id: `${chainInfo.chainId}/${padHex(transceiver.address, { dir: "left", size: 32 }).substring(2)}/${(topic.args as any).sequence.toString()}`,
                });
              }
            }
          }
        } catch (e) {}
      }
    }
    return supportedMessages;
  },
};

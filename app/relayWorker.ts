import { Worker } from "bullmq";
import { addRelayToQueue, getRelay, saveRelay } from "./redis";
import { ModularMessageRequest, VAAv1Request } from "./requestForExecution";
import { relayMM, relayVAAv1 } from "./relayHandlers";

const relayWorker = new Worker(
  "relay-queue",
  async (job) => {
    const { id } = job.data;
    const relay = await getRelay(id);

    if (!relay) {
      console.error(`Relay ${id} not found in Redis.`);
      return;
    }

    console.log(`Processing relay ${id}:`, relay);

    if (relay.instruction) {
      try {
        if (relay.instruction instanceof VAAv1Request) {
          const txs = await relayVAAv1(
            relay.requestForExecution,
            relay.instruction,
          );
          relay.status = "submitted";
          relay.txs.push(...txs);
        } else if (relay.instruction instanceof ModularMessageRequest) {
          const txs = await relayMM(
            relay.requestForExecution,
            relay.instruction,
          );
          relay.status = "submitted";
          relay.txs.push(...txs);
        } else {
          relay.status = "unsupported";
        }
      } catch (e: any) {
        console.error(`Error processing relay ${id}:`, e);
        if (e?.message?.includes("reverted")) {
          relay.status = "failed";
        } else {
          console.log(`Retrying relay ${id}...`);
          await addRelayToQueue(id); // Re-add job to queue
          return;
        }
      }
    } else {
      relay.status = "unsupported";
    }

    await saveRelay(id, relay); // Update status in Redis
    console.log(`Relay ${id} processed with status: ${relay.status}`);
  },
  { connection: { host: "127.0.0.1", port: 6379 } },
);

console.log("Relay worker is listening for jobs...");

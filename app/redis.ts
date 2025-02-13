import { Queue } from "bullmq";
import { redis } from "./redisClient";

const RELAYS_KEY = "relays";
const PENDING_RELAYS_KEY = "pending-relays";

export async function saveRelay(id: string, relayData: any) {
  console.log(`Relay ${id} saving to Redis`);
  await redis.set(`relay:${id}`, JSON.stringify(relayData));
  console.log(`Relay ${id} saved to Redis`);
}

export async function getRelay(id: string) {
  const data = await redis.get(`relay:${id}`);
  return data ? JSON.parse(data) : null;
}

export async function deleteRelay(id: string) {
  await redis.del(`relay:${id}`);
}

export const relayQueue = new Queue("relay-queue", {
  connection: { host: "127.0.0.1", port: 6379 },
});

/** Add a relay ID to the BullMQ queue */
export async function addRelayToQueue(id: string) {
  console.log(`Relay ${id} adding to queue...`);
  await relayQueue.add("process-relay", { id });
  console.log(`Relay ${id} added to queue`);
}
export { redis };

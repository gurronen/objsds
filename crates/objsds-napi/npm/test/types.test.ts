import {
  Objsds,
  type Ack,
  type LogRecord,
  type MessageId,
  type QueueClaim,
  type Version,
} from "../src/index.js";

interface User {
  name: string;
  active: boolean;
}

const client = Objsds.memory({ namespace: "type-test" });
const map = await client.map<User>("users", { schema: "user-v1" }).openOrCreate();
const user: User | undefined = await map.get("alice");
const version: Version = await map.insert("alice", { name: "Alice", active: true });
const log = await client.log<User>("audit", { schema: "audit-v1" }).openOrCreate();
const records: Array<LogRecord<User>> = await log.records();
const queue = await client.queue<User>("jobs", { schema: "job-v1" }).openOrCreate();
const messageId: MessageId = await queue.publish({ name: "Alice", active: true });
const claim: QueueClaim<User> | undefined = await queue.claim(1_000);
const ack: Ack | undefined = claim && await queue.ack(claim.id, claim.leaseToken);

void user;
void version;
void records;
void messageId;
void ack;

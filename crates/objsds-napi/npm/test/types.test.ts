import { Objsds, type LogRecord, type Version } from "../src/index.js";

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

void user;
void version;
void records;

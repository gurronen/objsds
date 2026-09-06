import assert from "node:assert/strict";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import test from "node:test";

import { Objsds, ObjsdsError, type MessageId } from "../src/index.js";

type User = {
  name: string;
  active: boolean;
};

test("creates, opens, and mutates a typed memory map", async () => {
  const client = Objsds.memory({ namespace: "tests" });
  const builder = client.map<User>("users", { schema: "user-json-v1" });
  const users = await builder.create();

  const version = await users.insert("alice", { name: "Alice", active: true });
  assert.equal(typeof version, "string");
  assert.deepEqual(await users.get("alice"), { name: "Alice", active: true });
  assert.deepEqual(await users.entries(), [["alice", { name: "Alice", active: true }]]);

  assert.deepEqual(
    await users.insertIfAbsent("alice", { name: "Replacement", active: false }),
    { inserted: false, value: { name: "Alice", active: true } },
  );
  assert.deepEqual(await users.remove("alice"), { name: "Alice", active: true });
  assert.equal(await users.get("alice"), undefined);

  const nullable = await client.map<null>("nullable", { schema: "null-v1" }).create();
  await nullable.insert("present", null);
  assert.equal(await nullable.get("present"), null);
  assert.equal(await nullable.get("absent"), undefined);
});

test("shares a memory store between handles from one client", async () => {
  const client = Objsds.memory({ namespace: "shared" });
  const first = await client.map<number>("counts", { schema: "count-v1" }).openOrCreate();
  const second = await client.map<number>("counts", { schema: "count-v1" }).open();

  await first.insert("total", 3);
  assert.equal(await second.get("total"), 3);
});

test("persists filesystem structures across clients", async () => {
  const root = await mkdtemp(join(tmpdir(), "objsds-node-"));
  try {
    const firstClient = Objsds.filesystem({ namespace: "persistent", root });
    const first = await firstClient.map<number>("counts", { schema: "count-v1" }).create();
    await first.insert("total", 7);

    const secondClient = Objsds.filesystem({ namespace: "persistent", root });
    const second = await secondClient.map<number>("counts", { schema: "count-v1" }).open();
    assert.equal(await second.get("total"), 7);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test("explicitly releases native Map, Log, and Queue handles", async () => {
  const client = Objsds.memory({ namespace: "cleanup" });
  const map = await client.map<number>("counts", { schema: "count-v1" }).create();
  const log = await client.log<number>("events", { schema: "event-v1" }).create();
  const queue = await client.queue<number>("jobs", { schema: "job-v1" }).create();

  map.close();
  map.close();
  log.close();
  log.close();
  queue.close();
  queue.close();

  await assert.rejects(map.get("total"), (error: unknown) => {
    assert.ok(error instanceof ObjsdsError);
    assert.equal(error.code, "ERR_OBJSDS_INVALID_HANDLE");
    return true;
  });
  await assert.rejects(log.records(), (error: unknown) => {
    assert.ok(error instanceof ObjsdsError);
    assert.equal(error.code, "ERR_OBJSDS_INVALID_HANDLE");
    return true;
  });
  await assert.rejects(queue.isEmpty(), (error: unknown) => {
    assert.ok(error instanceof ObjsdsError);
    assert.equal(error.code, "ERR_OBJSDS_INVALID_HANDLE");
    return true;
  });
});

test("appends and traverses a typed log", async () => {
  const client = Objsds.memory({ namespace: "logs" });
  const log = await client.log<User>("audit", { schema: "audit-v1" }).openOrCreate();

  const firstId = await log.append({ name: "Alice", active: true });
  const secondId = await log.append({ name: "Bob", active: false });
  assert.equal(typeof firstId, "string");
  assert.equal(typeof secondId, "string");

  assert.deepEqual(await log.get(firstId), {
    id: firstId,
    value: { name: "Alice", active: true },
  });
  assert.deepEqual(await log.recordsAfter(firstId), [
    { id: secondId, value: { name: "Bob", active: false } },
  ]);
});

test("publishes, claims, and acknowledges a typed queue", async () => {
  const client = Objsds.memory({ namespace: "queues" });
  const queue = await client.queue<User>("jobs", { schema: "job-v1" }).openOrCreate();

  const id = await queue.publish({ name: "Alice", active: true });
  assert.equal(typeof id, "string");
  assert.equal(await queue.len(), 1);
  assert.equal(await queue.isEmpty(), false);

  const claim = await queue.claim(1_000);
  assert.ok(claim);
  assert.equal(claim.id, id);
  assert.deepEqual(claim.value, { name: "Alice", active: true });
  assert.equal(claim.attempt, 1);
  assert.equal(typeof claim.leaseToken, "string");
  assert.equal(typeof claim.leaseExpiresAtMillis, "number");
  assert.equal(await queue.claim(1_000), undefined);
  await assert.rejects(
    queue.ack("invalid" as MessageId, claim.leaseToken),
    (error: unknown) => {
      assert.ok(error instanceof ObjsdsError);
      assert.equal(error.code, "ERR_OBJSDS_INVALID_MESSAGE_ID");
      return true;
    },
  );
  assert.equal(await queue.ack(claim.id, claim.leaseToken), "acknowledged");
  assert.equal(await queue.isEmpty(), true);
  assert.equal(await queue.ack(claim.id, claim.leaseToken), "notFound");
});

test("rejects invalid queue leases before native I/O", async () => {
  const queue = await Objsds.memory({ namespace: "queue-leases" })
    .queue<number>("jobs", { schema: "job-v1" })
    .create();

  assert.throws(() => queue.claim(0), /positive safe integer/);
  assert.throws(() => queue.claim(1.5), /positive safe integer/);
});

test("supports AbortSignal cancellation for queued native work", async () => {
  const client = Objsds.memory({ namespace: "abort" });
  const map = await client.map<number>("counts", { schema: "count-v1" }).create();
  const controller = new AbortController();
  controller.abort();

  await assert.rejects(map.get("total", { signal: controller.signal }), /abort/i);
  await assert.rejects(
    client.log<number>("events", { schema: "event-v1" }).create({ signal: controller.signal }),
    /abort/i,
  );
  await assert.rejects(
    client.queue<number>("jobs", { schema: "job-v1" }).create({ signal: controller.signal }),
    /abort/i,
  );
});

test("returns stable structured errors", async () => {
  const client = Objsds.memory({ namespace: "errors" });
  const builder = client.map<string>("missing", { schema: "text-v1" });

  await assert.rejects(builder.open(), (error: unknown) => {
    assert.ok(error instanceof ObjsdsError);
    assert.equal(error.code, "ERR_OBJSDS_NOT_FOUND");
    return true;
  });

  await builder.create();
  await assert.rejects(builder.create(), (error: unknown) => {
    assert.ok(error instanceof ObjsdsError);
    assert.equal(error.code, "ERR_OBJSDS_ALREADY_EXISTS");
    assert.equal(typeof error.details.observedVersion, "string");
    return true;
  });
});

test("rejects non-JSON input before crossing the native boundary", async () => {
  const client = Objsds.memory({ namespace: "json" });
  const map = await client.map<unknown>("values", { schema: "json-v1" }).create();

  await assert.rejects(map.insert("undefined", undefined), {
    name: "TypeError",
    message: /JSON-compatible/,
  });
  await assert.rejects(map.insert("bigint", 1n), {
    name: "TypeError",
    message: /JSON-compatible/,
  });
  await assert.rejects(map.insert("nested", { omitted: undefined }), {
    name: "TypeError",
    message: /JSON-compatible/,
  });
  await assert.rejects(map.insert("number", Number.POSITIVE_INFINITY), {
    name: "TypeError",
    message: /JSON-compatible/,
  });
});

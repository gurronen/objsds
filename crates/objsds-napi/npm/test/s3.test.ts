import assert from "node:assert/strict";
import test from "node:test";

import { Objsds, ObjsdsError } from "../src/index.js";

const enabled = process.env.OBJSDS_RUSTFS_E2E === "1";

test("persists Map and Log data through the RustFS S3 adapter", { skip: !enabled }, async () => {
  const namespace = `node-rustfs-${process.pid}-${Date.now()}`;
  const client = Objsds.s3({
    namespace,
    bucket: "objsds-e2e",
    region: "us-east-1",
    endpoint: "http://localhost:9000",
    pathStyle: true,
    credentials: {
      accessKeyId: "rustfsadmin",
      secretAccessKey: "rustfsadmin",
    },
  });

  const map = await client.map<number>("counts", { schema: "count-v1" }).create();
  await map.insert("total", 7);

  const log = await client.log<string>("events", { schema: "event-v1" }).create();
  const id = await log.append("created");

  const reopened = Objsds.s3({
    namespace,
    bucket: "objsds-e2e",
    region: "us-east-1",
    endpoint: "http://localhost:9000",
    pathStyle: true,
    credentials: {
      accessKeyId: "rustfsadmin",
      secretAccessKey: "rustfsadmin",
    },
  });
  assert.equal(await (await reopened.map<number>("counts", { schema: "count-v1" }).open()).get("total"), 7);
  assert.deepEqual(await (await reopened.log<string>("events", { schema: "event-v1" }).open()).get(id), {
    id,
    value: "created",
  });
});

test("surfaces CAS conflicts under S3 write contention without retrying", { skip: !enabled }, async () => {
  const client = Objsds.s3({
    namespace: `node-contention-${process.pid}-${Date.now()}`,
    bucket: "objsds-e2e",
    region: "us-east-1",
    endpoint: "http://localhost:9000",
    pathStyle: true,
    credentials: { accessKeyId: "rustfsadmin", secretAccessKey: "rustfsadmin" },
  });
  const map = await client.map<number>("shared", { schema: "count-v1" }).create();

  const results = await Promise.allSettled(
    Array.from({ length: 32 }, (_, index) => map.insert(`key-${index}`, index)),
  );
  const conflicts = results.filter(
    (result): result is PromiseRejectedResult =>
      result.status === "rejected" &&
      result.reason instanceof ObjsdsError &&
      result.reason.code === "ERR_OBJSDS_CONFLICT",
  );
  const successes = results.filter((result) => result.status === "fulfilled");

  assert.ok(successes.length > 0, "at least one contender should win");
  assert.ok(conflicts.length > 0, "concurrent writes should expose at least one CAS conflict");
  assert.equal((await map.entries()).length, successes.length);
});

import assert from "node:assert/strict";
import test from "node:test";

import { Objsds } from "../src/index.js";

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

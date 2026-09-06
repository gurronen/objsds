import assert from 'node:assert/strict'
import { Objsds } from '../dist/index.js'

const client = Objsds.memory({ namespace: 'package-smoke' })
const map = await client.map('values', { schema: 'json-v1' }).openOrCreate()
await map.insert('answer', 42)
assert.equal(await map.get('answer'), 42)

const queue = await client.queue('jobs', { schema: 'json-v1' }).openOrCreate()
const id = await queue.publish({ answer: 42 })
const claim = await queue.claim(1000)
assert.ok(claim)
assert.equal(claim.id, id)
assert.deepEqual(claim.value, { answer: 42 })
assert.equal(await queue.ack(claim.id, claim.leaseToken), 'acknowledged')

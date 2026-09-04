import assert from 'node:assert/strict'
import { Objsds } from '../dist/index.js'

const client = Objsds.memory({ namespace: 'package-smoke' })
const map = await client.map('values', { schema: 'json-v1' }).openOrCreate()
await map.insert('answer', 42)
assert.equal(await map.get('answer'), 42)

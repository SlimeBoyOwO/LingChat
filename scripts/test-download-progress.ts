import assert from 'node:assert/strict'

const module = await import('../src/api/services/download-progress.ts')
const { createProgressBus, ProgressBus } = module

const bus: ProgressBus = createProgressBus()
const seen: number[] = []
const unsub = bus.subscribe((p) => seen.push(p.percent))
assert.equal(bus.listenerCount, 1)

bus.dispatch({ asset_id: 'deberta', bytes_done: 1, total_bytes: 100, percent: 1 })
bus.dispatch({ asset_id: 'deberta', bytes_done: 50, total_bytes: 100, percent: 50 })
bus.dispatch({ asset_id: 'ling-v2', bytes_done: 5, total_bytes: 10, percent: 50 })

assert.deepEqual(seen, [1, 50, 50])

unsub()
assert.equal(bus.listenerCount, 0)
bus.dispatch({ asset_id: 'deberta', bytes_done: 100, total_bytes: 100, percent: 100 })
assert.deepEqual(seen, [1, 50, 50])

console.log('Download progress bus tests passed')

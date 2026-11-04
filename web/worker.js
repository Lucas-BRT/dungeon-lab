// One worker of the pool: its own instance of the module (own memory and allocator), asked for slices of a batch.
import init, { bench } from './pkg/dungeon_lab.js'

const ready = init()

self.onmessage = async ({ data: { id, level, from, count } }) => {
  await ready
  self.postMessage({ id, ...JSON.parse(bench(level, from, count)) })
}

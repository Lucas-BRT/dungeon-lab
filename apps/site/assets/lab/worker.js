// One worker of the lab's pool: its own instance of the module (own memory and allocator), asked for slices of a batch.
// The module lives at a fixed path, outside the hashed assets, because the glue finds its `.wasm` next to itself.
import init, { bench } from '/lab/pkg/tapiti.js'

const ready = init()

self.onmessage = async ({ data: { id, size, from, count } }) => {
  await ready
  self.postMessage({ id, ...JSON.parse(bench(size, from, count)) })
}

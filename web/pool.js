/**
 * A pool of Web Workers, each with its own single-threaded instance of the module. A stage is cheap (a few ms), so the parallelism that pays in
 * the browser is across stages: a batch is cut into slices, one per worker, with no shared memory and no special headers on the page.
 * (A build of the module with threads over one shared memory was tried: the generator allocates a lot, and the allocator's lock made it
 * about 16 times slower than the single-threaded one.)
 */
export const createPool = (size = Math.max(1, Math.min(navigator.hardwareConcurrency || 4, 16))) => {
  const workers = Array.from({ length: size }, () => new Worker(new URL('./worker.js', import.meta.url), { type: 'module' }))
  let next = 0
  const pending = new Map()
  for (const w of workers) {
    w.onmessage = ({ data }) => {
      pending.get(data.id)?.(data)
      pending.delete(data.id)
    }
  }
  /** `count` stages of `level` from seed `from`, cut into one slice per worker (or fewer): `{ invalid, ms (wall clock), workers }`. */
  const batch = async (level, from, count, use = size) => {
    const t0 = performance.now()
    const n = Math.max(1, Math.min(use, size, count))
    const slices = Array.from({ length: n }, (_, i) => [from + Math.floor((count * i) / n), Math.floor((count * (i + 1)) / n) - Math.floor((count * i) / n)])
    const answers = await Promise.all(
      slices.map(
        ([f, c], i) =>
          new Promise((resolve) => {
            const id = next++
            pending.set(id, resolve)
            workers[i].postMessage({ id, level, from: f, count: c })
          }),
      ),
    )
    return { invalid: answers.reduce((a, r) => a + r.invalid, 0), ms: performance.now() - t0, workers: n }
  }
  return { size, batch, close: () => workers.forEach((w) => w.terminate()) }
}

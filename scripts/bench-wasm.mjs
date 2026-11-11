// Times the single-threaded modules in Node (no browser needed): `node scripts/bench-wasm.mjs [level] [stages]`.
import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
const level = Number(process.argv[2] ?? 5)
const n = Number(process.argv[3] ?? 60)
for (const dir of ['pkg', 'pkg-simd']) {
  const m = await import(pathToFileURL(new URL(`../apps/site/${dir}/tapiti.js`, import.meta.url).pathname).href)
  m.initSync({ module: readFileSync(new URL(`../apps/site/${dir}/tapiti_bg.wasm`, import.meta.url)) })
  m.bench(level, 1, 10) // warm up
  const runs = Array.from({ length: 5 }, () => JSON.parse(m.bench(level, 1, n)).ms / n)
  console.log(`${dir.padEnd(11)} level ${level}: best ${Math.min(...runs).toFixed(2)} ms · median ${runs.sort((a, b) => a - b)[2].toFixed(2)} ms per stage`)
}

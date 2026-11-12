// Checks the pure geometry of the landing page's walk (apps/site/assets/walk/build.js) against the real generator, without a browser.
//   scripts/build-wasm.sh && node scripts/walk-check.mjs
// The seed, the size and the stations must match apps/site/src/floor.rs (`WALK_SEED`, `WALK_SIZE_INDEX`, `stations()`); that file's tests pin
// the Rust side, the numbers below pin this side. If the generator changes the floor of the walk, both fail and say so.
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { pathToFileURL } from 'node:url'
import { resolve } from 'node:path'

const WALK_SEED = 279
const WALK_SIZE_INDEX = 2
const STATION_ROOMS = [6, 7, 1, 14, 11, 12, 0] // floor.rs: stations()[i].room

const root = resolve(import.meta.dirname, '..')
const wasm = await import(pathToFileURL(resolve(root, 'apps/site/pkg/tapiti.js')).href)
wasm.initSync({ module: readFileSync(resolve(root, 'apps/site/pkg/tapiti_bg.wasm')) })
const { build, WALL_HEIGHT } = await import(pathToFileURL(resolve(root, 'apps/site/assets/walk/build.js')).href)

const answer = JSON.parse(wasm.stage_json(WALK_SEED, wasm.rich_params(WALK_SIZE_INDEX)))
assert.deepEqual(answer.violations, [], 'the walk floor must be valid')
const floor = answer.floor

const t0 = performance.now()
const scene = build(floor)
const ms = performance.now() - t0

// Stations: the same rooms, in the same order, as the server's.
assert.deepEqual(scene.stations.map((s) => s.room), STATION_ROOMS)
for (let i = 1; i < scene.stations.length; i++) assert.ok(scene.stations[i].u > scene.stations[i - 1].u, 'stations follow the path')
assert.equal(scene.stations[0].u, 0)
assert.ok(Math.abs(scene.stations.at(-1).u - 1) < 1e-6, 'the last station is the end of the path')
assert.equal(floor.rooms[scene.stations[0].room].role, 'start')
assert.equal(floor.rooms[scene.stations.at(-1).room].role, 'stairs')

// The path is continuous: no hop longer than a room's width.
const p = scene.path.points
let longest = 0
for (let i = 3; i < p.length; i += 3) longest = Math.max(longest, Math.hypot(p[i] - p[i - 3], p[i + 2] - p[i - 1]))
assert.ok(longest < 700, `path hop of ${longest.toFixed(0)} units`)

// Budgets.
const triangles = scene.walls.indices.length / 3
assert.ok(triangles > 200 && triangles <= 15000, `${triangles} wall triangles`)
const torches = scene.torches.length / 4
assert.ok(torches >= 20 && torches <= 60, `${torches} torches`)
assert.ok(scene.contours >= 1)
assert.ok(ms < 60, `build took ${ms.toFixed(1)} ms`)

// Walls are inside the floor's bounds, and no taller than the wall height.
const [x0, y0, x1, y1] = scene.bounds
for (let i = 0; i < scene.walls.positions.length; i += 3) {
  const [x, h, z] = [scene.walls.positions[i], scene.walls.positions[i + 1], scene.walls.positions[i + 2]]
  assert.ok(x >= x0 - 12 && x <= x1 + 12 && z >= y0 - 12 && z <= y1 + 12, 'wall inside the bounds')
  assert.ok(h >= 0 && h <= WALL_HEIGHT)
}

// Deterministic: building twice gives identical bytes.
const again = build(floor)
const same = (a, b) => Buffer.compare(Buffer.from(a.buffer, a.byteOffset, a.byteLength), Buffer.from(b.buffer, b.byteOffset, b.byteLength)) === 0
assert.ok(same(scene.walls.positions, again.walls.positions) && same(scene.walls.indices, again.walls.indices), 'walls differ between builds')
assert.ok(same(scene.torches, again.torches) && same(scene.path.points, again.path.points), 'torches or path differ between builds')

console.log(`walk ok: seed ${WALK_SEED} size ${WALK_SIZE_INDEX}, ${scene.contours} contours, ${triangles} wall triangles, ${torches} torches, ${(scene.path.total).toFixed(0)} units of path, built in ${ms.toFixed(1)} ms`)

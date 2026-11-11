// Smoke test of the landing page's walk in headless Firefox, over WebDriver BiDi (Node 22+ and `firefox`; start the site first).
//   node scripts/site-smoke.mjs [base-url]        default: http://127.0.0.1:3000
// Checks the scene starts and stays inside its budgets, follows the scroll, agrees with the server's minimap, does not shift the layout,
// can be switched off, steps under reduced motion, and that the docs load neither three.js nor wasm.
import { spawn } from 'node:child_process'
import { mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const base = process.argv[2] ?? 'http://127.0.0.1:3000'
const port = 9336
const profile = mkdtempSync(join(tmpdir(), 'tapiti-site-'))
const firefox = spawn('firefox', ['--headless', '--no-remote', '--profile', profile, `--remote-debugging-port=${port}`], { stdio: 'ignore' })
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
let ws
for (let i = 0; i < 60 && !ws; i++) {
  try {
    ws = await new Promise((resolve, reject) => { const s = new WebSocket(`ws://127.0.0.1:${port}/session`); s.onopen = () => resolve(s); s.onerror = reject })
  } catch { await sleep(500) }
}
if (!ws) { firefox.kill(); console.error('could not reach Firefox'); process.exit(2) }
let id = 0
const waiting = new Map()
const errors = []
ws.onmessage = ({ data }) => {
  const m = JSON.parse(data)
  if (m.id && waiting.has(m.id)) waiting.get(m.id)(m)
  else if (m.method === 'log.entryAdded' && m.params.level === 'error') errors.push(m.params.text)
}
const send = (method, params = {}) => new Promise((resolve) => { const n = ++id; waiting.set(n, resolve); ws.send(JSON.stringify({ id: n, method, params })) })
await send('session.new', { capabilities: {} })
await send('session.subscribe', { events: ['log.entryAdded'] })
const { result: { contexts: [{ context }] } } = await send('browsingContext.getTree', {})
await send('browsingContext.setViewport', { context, viewport: { width: 1440, height: 900 } })
const open = async (path) => { await send('browsingContext.navigate', { context, url: base + path, wait: 'complete' }) }
const run = async (body) => {
  const { result } = await send('script.evaluate', { expression: `(async () => { ${body} })()`, target: { context }, awaitPromise: true })
  return result.type === 'exception' ? `EXCEPTION ${result.exceptionDetails.text}` : result.result?.value
}
const until = async (body, ms = 20000) => { for (let t = 0; t < ms; t += 250) { if (await run(body)) return true; await sleep(250) } return false }

let failed = 0
const check = (name, ok, detail = '') => { if (!ok) failed++; console.log(`${ok ? 'ok  ' : 'FAIL'} ${name}${ok ? '' : `: ${detail}`}`) }

// The layout without the scene: the reference for "no layout shift".
await open('/?scene=off')
await sleep(800)
const h1Off = await run("return document.querySelector('h1').getBoundingClientRect().top")
check('?scene=off keeps the map', (await run("return !document.documentElement.classList.contains('walk-on') && !window.__walk")) === true)

await open('/')
const started = await until("return document.documentElement.classList.contains('walk-on')")
check('the scene starts', started)
const w = JSON.parse((await run('return JSON.stringify(window.__walk && { ...window.__walk, stations: window.__walk.stations.map((s) => [s.x, s.y, s.u]) })')) ?? 'null') ?? {}
check('draw calls <= 40', w.calls <= 40, w.calls)
check('triangles <= 60000', w.triangles <= 60000, w.triangles)
check('lights: 6 pool + carried + below + hemisphere', w.lights <= 9, w.lights)
const h1On = await run("return document.querySelector('h1').getBoundingClientRect().top")
check('the scene does not move the page', Math.abs(h1On - h1Off) < 0.5, `${h1Off} vs ${h1On}`)

const stops = JSON.parse(await run("return JSON.stringify([...document.querySelectorAll('.minimap .stop')].map((a) => [Number(a.dataset.x), Number(a.dataset.y)]))"))
check('minimap stops match the scene stations', w.stations && stops.length === 7 && stops.every(([x, y], i) => Math.abs(x - w.stations[i][0]) < 0.1 && Math.abs(y - w.stations[i][1]) < 0.1), JSON.stringify(stops))

for (const [i, section] of ['entrance', 'shapes', 'gate', 'dig', 'numbers', 'run', 'stairs'].entries()) {
  await run(`const s = document.getElementById('${section}'); window.scrollTo({ top: s.getBoundingClientRect().top + scrollY - innerHeight * 0.5 + 2, behavior: 'instant' })`)
  const reached = await until(`return Math.abs(window.__walk.camera.u - ${w.stations?.[i]?.[2] ?? 0}) < 0.02`, 8000)
  check(`scrolling to "${section}" brings the camera there`, reached, await run('return JSON.stringify(window.__walk.camera)'))
}
const marked = await run("return document.querySelector('.minimap .stop[aria-current]')?.getAttribute('href')")
check('the minimap and gauge follow the room', marked === '#stairs', marked)

await open('/?motion=reduce')
await until("return document.documentElement.classList.contains('walk-on')")
check('reduced motion steps the camera', (await run('return window.__walk && window.__walk.stepped')) === true)

await open('/docs/model')
await sleep(1500)
const heavy = await run("return performance.getEntriesByType('resource').map((r) => r.name).filter((n) => /three-subset|tapiti_bg\\.wasm|scene\\.js/.test(n)).length")
check('docs load no three.js and no wasm', heavy === 0, heavy)

if (errors.length) { failed++; console.log('FAIL console errors:', errors.slice(0, 5)) }
ws.close()
firefox.kill()
process.exit(failed ? 1 : 0)

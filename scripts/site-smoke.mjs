// Smoke test of the landing page's walk in headless Firefox, over WebDriver BiDi (Node 22+ and `firefox`; start the site first).
//   node scripts/site-smoke.mjs [base-url]        default: http://127.0.0.1:3000
// Checks the scene starts and stays inside its budgets, follows the scroll, agrees with the server's minimap, does not shift the layout,
// can be switched off, steps under reduced motion, and that the docs load neither three.js nor wasm. With AXE_JS set, also runs axe-core.
import { spawn } from 'node:child_process'
import { readFileSync } from 'node:fs'
import { freshProfile } from './firefox-profile.mjs'

const base = process.argv[2] ?? 'http://127.0.0.1:3000'
const port = 9336
const profile = freshProfile('site')
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

const webgl = await run("const c = document.createElement('canvas'); let why = ''; c.addEventListener('webglcontextcreationerror', (e) => { why = e.statusMessage }); const gl = c.getContext('webgl2'); return gl ? 'yes' : 'no: ' + (why || 'no reason given')")
check('WebGL2 is available in this Firefox', webgl === 'yes', `${webgl}; the scene and the lab 3D view need it`)
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

// A dead end, and every address that leads nowhere, in the same room (with the right status).
await open('/leaf')
const robots = await run("return document.querySelector('meta[name=robots]')?.content")
check('/leaf is noindex', /noindex/.test(robots ?? ''), robots)
const leafOn = await until("return document.documentElement.classList.contains('walk-on')")
check('the leaf chamber draws its room', leafOn && (await run('return window.__walk && window.__walk.poolSize')) === 1)
for (const path of ['/nope', '/a/b/c', '/docs/nope']) {
  const status = await run(`return (await fetch('${path}')).status`)
  const title = await run(`return (await (await fetch('${path}')).text()).includes('Nothing here')`)
  check(`${path} is a 404 with the leaf chamber`, status === 404 && title === true, `${status}`)
}
await open('/leaf?motion=reduce')
await until("return document.documentElement.classList.contains('walk-on')")
check('reduced motion keeps the leaf chamber still', (await run('return window.__walk && window.__walk.stepped')) === true)

await open('/docs/model')
await sleep(1500)
const heavy = await run("return performance.getEntriesByType('resource').map((r) => r.name).filter((n) => /three-subset|tapiti_bg\\.wasm|scene\\.js/.test(n)).length")
check('docs load no three.js and no wasm', heavy === 0, heavy)

// Accessibility, when axe-core is at hand: AXE_JS=/path/to/axe.min.js (CI installs it into a temp dir). No serious or critical violation.
if (process.env.AXE_JS) {
  const axe = readFileSync(process.env.AXE_JS, 'utf8')
  for (const path of ['/', '/docs/model', '/search?q=seed', '/leaf', '/lab']) {
    await open(path)
    await sleep(path === '/lab' || path === '/' || path === '/leaf' ? 5000 : 1000)
    const found = await run(`${axe}\n; const r = await axe.run(document, { runOnly: ['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'] }); return JSON.stringify(r.violations.filter((v) => v.impact === 'serious' || v.impact === 'critical').map((v) => v.id + ' x' + v.nodes.length))`)
    const list = JSON.parse(found ?? '["no result"]')
    check(`axe: no serious or critical violation on ${path}`, Array.isArray(list) && list.length === 0, found)
  }
}

if (errors.length) { failed++; console.log('FAIL console errors:', errors.slice(0, 5)) }
ws.close()
firefox.kill()
process.exit(failed ? 1 : 0)

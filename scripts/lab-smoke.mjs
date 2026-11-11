// Smoke test of the lab in headless Firefox, over WebDriver BiDi (no driver to install; needs Node 22+ and `firefox`).
//   node scripts/lab-smoke.mjs [url]        default: http://127.0.0.1:3000/lab (start the site first: topcoat dev -p tapiti-site)
// Checks that the floor is drawn and the panel filled, that editing a parameter and a bad value behave, and that a benchmark runs.
import { spawn } from 'node:child_process'
import { mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const url = process.argv[2] ?? 'http://127.0.0.1:3000/lab'
const port = 9333
const profile = mkdtempSync(join(tmpdir(), 'tapiti-lab-'))
const firefox = spawn('firefox', ['--headless', '--no-remote', '--profile', profile, `--remote-debugging-port=${port}`], { stdio: 'ignore' })
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))

let ws
for (let i = 0; i < 60 && !ws; i++) {
  try {
    ws = await new Promise((resolve, reject) => {
      const socket = new WebSocket(`ws://127.0.0.1:${port}/session`)
      socket.onopen = () => resolve(socket)
      socket.onerror = reject
    })
  } catch { await sleep(500) }
}
if (!ws) { firefox.kill(); console.error('could not reach Firefox'); process.exit(2) }

let id = 0
const waiting = new Map()
const logs = []
ws.onmessage = ({ data }) => {
  const message = JSON.parse(data)
  if (message.id && waiting.has(message.id)) waiting.get(message.id)(message)
  else if (message.method === 'log.entryAdded' && message.params.level === 'error') logs.push(message.params.text)
}
const send = (method, params = {}) => new Promise((resolve) => { const n = ++id; waiting.set(n, resolve); ws.send(JSON.stringify({ id: n, method, params })) })

await send('session.new', { capabilities: {} })
await send('session.subscribe', { events: ['log.entryAdded'] })
const { result: { contexts: [{ context }] } } = await send('browsingContext.getTree', {})
await send('browsingContext.setViewport', { context, viewport: { width: 1440, height: 900 } })
await send('browsingContext.navigate', { context, url, wait: 'complete' })
await sleep(2500)
const resize = (width, height) => send('browsingContext.setViewport', { context, viewport: { width, height } })
const run = async (body) => {
  const { result } = await send('script.evaluate', { expression: `(async () => { ${body} })()`, target: { context }, awaitPromise: true })
  return result.type === 'exception' ? `EXCEPTION ${result.exceptionDetails.text}` : result.result?.value
}

const checks = [
  ['panel is filled', `return document.getElementById('results').innerText.includes('Valid map')`, (v) => v === true],
  ['level button works', `document.querySelector('[data-level="3"]').click(); await new Promise(r => setTimeout(r, 700)); return document.getElementById('results').innerText.includes('Level 3, seed')`, (v) => v === true],
  ['a parameter change is counted', `const el = document.querySelector('[name="cols"]'); el.value = '9'; el.dispatchEvent(new Event('change', { bubbles: true })); return document.getElementById('changed-status').textContent`, (v) => /1 parameter/.test(v)],
  ['a bad value is refused', `const el = document.querySelector('[name="rows"]'); el.value = '999'; el.dispatchEvent(new Event('change', { bubbles: true })); return el.parentElement.querySelector('.field-error').textContent`, (v) => v.length > 0],
  ['benchmark runs', `document.getElementById('bench-40').click(); return document.getElementById('bench-status').textContent`, (v) => /invalid of 40/.test(v)],
]
let failed = 0
const report = (name, pass, value) => {
  if (!pass) failed++
  console.log(`${pass ? 'ok  ' : 'FAIL'} ${name}${pass ? '' : `: ${JSON.stringify(value)}`}`)
}
for (const [name, body, ok] of checks) {
  const value = await run(body)
  report(name, ok(value), value)
}

// The reading drawer (below 1280px): opens, takes the focus, closes on Escape and gives the focus back.
await resize(1100, 800)
await sleep(300)
report('the reading drawer opens', (await run("document.getElementById('reading-toggle').click(); await new Promise(r => setTimeout(r, 300)); return document.getElementById('reading-toggle').getAttribute('aria-expanded') + '|' + (document.activeElement?.id)")) === 'true|results', 'drawer')
report('Escape closes the drawer and returns the focus', (await run("document.getElementById('lab').dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true })); await new Promise(r => setTimeout(r, 300)); return document.getElementById('reading-toggle').getAttribute('aria-expanded') + '|' + (document.activeElement?.id)")) === 'false|reading-toggle', 'escape')

// The 3D view: mounts a scene, redraws on regenerate, and leaving it disposes the scene.
await resize(1440, 900)
await sleep(300)
await run("document.getElementById('view-3d').click()")
const mounted = await (async () => { for (let i = 0; i < 40; i++) { if (await run('return !!window.__walk && window.__walk.calls > 0')) return true; await sleep(500) } return false })()
report('the 3D view draws the floor', mounted, await run('return JSON.stringify(window.__walk)'))
report('the 3D view hides the plan tools', (await run("return document.getElementById('floor-map').hidden && !document.getElementById('floor-3d').hidden && document.getElementById('zoom-in').hidden")) === true, '3d')
await run("document.querySelector('[data-level=\"2\"]').click()")
await sleep(1500)
report('the 3D view follows a new floor', (await run('return !!window.__walk && window.__walk.calls > 0')) === true, 'regenerate')
await run("document.getElementById('view-plan').click()")
await sleep(500)
report('leaving 3D disposes the scene', (await run("return window.__walk === null && !document.getElementById('floor-map').hidden")) === true, 'dispose')

// No sideways scroll on a phone.
await resize(390, 844)
await sleep(600)
report('no horizontal scroll at 390px', (await run('return document.documentElement.scrollWidth <= document.documentElement.clientWidth')) === true, 'overflow')

// The old address still leads here.
const moved = await fetch(new URL('/en/lab', url), { redirect: 'manual' })
report('/en/lab redirects to /lab', moved.status === 308 && new URL(moved.headers.get('location'), url).pathname === '/lab', moved.status)
if (logs.length) { failed++; console.log('FAIL console errors:', logs) }
ws.close()
firefox.kill()
process.exit(failed ? 1 : 0)

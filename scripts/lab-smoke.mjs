// Smoke test of the lab in headless Firefox, over WebDriver BiDi (no driver to install; needs Node 22+ and `firefox`).
//   node scripts/lab-smoke.mjs [url]        default: http://127.0.0.1:3000/en/lab (start the site first: topcoat dev -p tapiti-site)
// Checks that the floor is drawn and the panel filled, that editing a parameter and a bad value behave, and that a benchmark runs.
import { spawn } from 'node:child_process'
import { mkdtempSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const url = process.argv[2] ?? 'http://127.0.0.1:3000/en/lab'
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
await send('browsingContext.navigate', { context, url, wait: 'complete' })
await sleep(2500)
const run = async (body) => {
  const { result } = await send('script.evaluate', { expression: `(async () => { ${body} })()`, target: { context }, awaitPromise: true })
  return result.type === 'exception' ? `EXCEPTION ${result.exceptionDetails.text}` : result.result?.value
}

const checks = [
  ['panel is filled', `return document.getElementById('results').innerText.includes('Valid map')`, (v) => v === true],
  ['level button works', `document.querySelector('[data-level="3"]').click(); await new Promise(r => setTimeout(r, 700)); return document.getElementById('results').innerText.includes('LEVEL 3')`, (v) => v === true],
  ['a parameter change is counted', `const el = document.querySelector('[name="cols"]'); el.value = '9'; el.dispatchEvent(new Event('change', { bubbles: true })); return document.getElementById('changed-status').textContent`, (v) => /1 parameter/.test(v)],
  ['a bad value is refused', `const el = document.querySelector('[name="rows"]'); el.value = '999'; el.dispatchEvent(new Event('change', { bubbles: true })); return el.parentElement.querySelector('.field-error').textContent`, (v) => v.length > 0],
  ['benchmark runs', `document.getElementById('bench-40').click(); return document.getElementById('bench-status').textContent`, (v) => /invalid of 40/.test(v)],
]
let failed = 0
for (const [name, body, ok] of checks) {
  const value = await run(body)
  const pass = ok(value)
  if (!pass) failed++
  console.log(`${pass ? 'ok  ' : 'FAIL'} ${name}${pass ? '' : `: ${JSON.stringify(value)}`}`)
}
if (logs.length) { failed++; console.log('FAIL console errors:', logs) }
ws.close()
firefox.kill()
process.exit(failed ? 1 : 0)

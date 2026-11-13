// Screenshots of the site's pages at desktop and phone width, in headless Firefox over WebDriver BiDi (Node 22+ and `firefox`).
//   node scripts/site-shots.mjs <out-dir> [base-url] [path ...]
// VIEWPORT=1 captures only the first screen. Default base http://127.0.0.1:3000 and paths / /docs/model /search?q=seed /leaf /lab. Writes <out-dir>/<name>-<width>.png.
import { spawn } from 'node:child_process'
import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { freshProfile } from './firefox-profile.mjs'

const [out, base = 'http://127.0.0.1:3000', ...rest] = process.argv.slice(2)
if (!out) { console.error('usage: node scripts/site-shots.mjs <out-dir> [base-url] [path ...]'); process.exit(2) }
const paths = rest.length ? rest : ['/', '/docs/model', '/search?q=seed', '/leaf', '/lab']
// WIDTHS=1440x900,1100x800 picks other viewports.
const widths = (process.env.WIDTHS ?? '1440x900,390x844').split(',').map((w) => w.split('x').map(Number))
mkdirSync(out, { recursive: true })

const port = 9334
const profile = freshProfile('shots')
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
ws.onmessage = ({ data }) => { const m = JSON.parse(data); if (m.id && waiting.has(m.id)) waiting.get(m.id)(m) }
const send = (method, params = {}) => new Promise((resolve) => { const n = ++id; waiting.set(n, resolve); ws.send(JSON.stringify({ id: n, method, params })) })

await send('session.new', { capabilities: {} })
const { result: { contexts: [{ context }] } } = await send('browsingContext.getTree', {})
for (const path of paths) {
  const name = path.replace(/[^a-z0-9]+/gi, '-').replace(/^-|-$/g, '') || 'home'
  for (const [width, height] of widths) {
    await send('browsingContext.setViewport', { context, viewport: { width, height } })
    await send('browsingContext.navigate', { context, url: base + path, wait: 'complete' })
    await sleep(path.startsWith('/lab') ? 3000 : 1200)
    const shot = await send('browsingContext.captureScreenshot', { context, origin: process.env.VIEWPORT ? 'viewport' : 'document' })
    writeFileSync(join(out, `${name}-${width}.png`), Buffer.from(shot.result.data, 'base64'))
    console.log(`${name}-${width}.png`)
  }
}
ws.close()
firefox.kill()

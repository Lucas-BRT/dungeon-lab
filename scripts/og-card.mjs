// Makes the social card, apps/site/assets/brand/og.png (1200x630): the bird's-eye frame of the landing page's walk, captured from the
// scene itself in headless Firefox over WebDriver BiDi, with the wordmark and the claim set in the site's own fonts.
//   node scripts/og-card.mjs [base-url] [out.png]       default: http://127.0.0.1:3000, apps/site/assets/brand/og.png (site running)
import { spawn } from 'node:child_process'
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join, resolve } from 'node:path'

const base = process.argv[2] ?? 'http://127.0.0.1:3000'
const out = resolve(process.argv[3] ?? join(import.meta.dirname, '../apps/site/assets/brand/og.png'))
const port = 9337
const profile = mkdtempSync(join(tmpdir(), 'tapiti-og-'))
const firefox = spawn('firefox', ['--headless', '--no-remote', '--profile', profile, `--remote-debugging-port=${port}`], { stdio: 'ignore' })
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms))
let ws
for (let i = 0; i < 60 && !ws; i++) {
  try {
    ws = await new Promise((res, rej) => { const s = new WebSocket(`ws://127.0.0.1:${port}/session`); s.onopen = () => res(s); s.onerror = rej })
  } catch { await sleep(500) }
}
if (!ws) { firefox.kill(); console.error('could not reach Firefox'); process.exit(2) }
let id = 0
const waiting = new Map()
ws.onmessage = ({ data }) => { const m = JSON.parse(data); if (m.id && waiting.has(m.id)) waiting.get(m.id)(m) }
const send = (method, params = {}) => new Promise((res) => { const n = ++id; waiting.set(n, res); ws.send(JSON.stringify({ id: n, method, params })) })
await send('session.new', { capabilities: {} })
const { result: { contexts: [{ context }] } } = await send('browsingContext.getTree', {})
// The card is the top-left 1200x630 of the page, whatever the window size is.
await send('browsingContext.setViewport', { context, viewport: { width: 1200, height: 630 } })
await send('browsingContext.navigate', { context, url: `${base}/`, wait: 'complete' })
const run = async (body) => (await send('script.evaluate', { expression: `(async () => { ${body} })()`, target: { context }, awaitPromise: true })).result?.result?.value
for (let i = 0; i < 60 && !(await run("return document.documentElement.classList.contains('walk-on')")); i++) await sleep(500)
await sleep(2500) // the torches finish lighting
await run(`
  const style = document.createElement('style')
  style.textContent = '.site-header, .site-footer, .rooms, .depth, .minimap { visibility: hidden !important } .walk-backdrop { inset: 0 auto auto 0 !important; width: 1200px !important; height: 630px !important } .walk-backdrop canvas { filter: brightness(2.2) contrast(1.15) saturate(1.15); transform: translate(13%, -6%) scale(1.12) } #card { position: fixed; z-index: 5; left: 64px; top: 380px; display: grid; gap: 14px } #card b { font: 700 120px/1 var(--font-display); letter-spacing: -.02em; color: var(--bone); text-shadow: 0 2px 24px #0b0e12 } #card span { font: 500 34px/1.2 var(--font-body); color: var(--ash); text-shadow: 0 2px 16px #0b0e12 } #card img { width: 72px; height: 72px }'
  document.head.append(style)
  const card = document.createElement('div')
  card.id = 'card'
  card.innerHTML = '<img src="/_topcoat/assets/' + '" alt=""><b>tapiti</b><span>Floors that always fit.</span>'
  card.querySelector('img').remove()
  document.body.append(card)
  window.scrollTo(0, 0)
  await new Promise((r) => setTimeout(r, 1200))
`)
const shot = await send('browsingContext.captureScreenshot', { context, origin: 'viewport', clip: { type: 'box', x: 0, y: 0, width: 1200, height: 630 } })
writeFileSync(out, Buffer.from(shot.result.data, 'base64'))
console.log(`wrote ${out}`)
ws.close()
firefox.kill()

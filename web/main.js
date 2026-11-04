import init, { default_params, stage_json, bench } from './pkg/dungeon_lab.js'

/**
 * The page of the dungeon lab: the generator, validator and measures run in the WebAssembly module (`src/`, Rust); this file only edits
 * Λ, asks for a stage and draws the answer. Drag pans, the wheel zooms, a double click fits; keys 1 to 5 pick a level, R a new seed.
 */

await init()

const CLASSES = ['rect', 'disc', 'oct', 'L', 'star']
const COLORS = { rect: '#3d5a80', disc: '#2a9d8f', oct: '#8e6cc9', L: '#c07a2e', star: '#b8a030' }
const ROLE = { start: '#6f6', stairs: '#f6f', ambush: '#f55', trap: '#f44', prize: '#fd5', empty: '#999' }
const DIRS = [[1, 0], [Math.SQRT1_2, Math.SQRT1_2], [0, 1], [-Math.SQRT1_2, Math.SQRT1_2], [-1, 0], [-Math.SQRT1_2, -Math.SQRT1_2], [0, -1], [Math.SQRT1_2, -Math.SQRT1_2]]

const state = { level: 1, seed: 1, P: JSON.parse(default_params(1)) }
const overlays = { cells: true, labels: true, ports: false, pairs: false, candidates: false }
const view = { zoom: 1, x: 0, y: 0 }
let current = null

const h = (tag, attrs = {}, ...kids) => {
  const el = document.createElement(tag)
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v)
  el.append(...kids)
  return el
}

/* ---------------- Parameters ---------------- */

const mixKey = (a, b) => (CLASSES.indexOf(a) <= CLASSES.indexOf(b) ? `${a}-${b}` : `${b}-${a}`)

const GROUPS = [
  { title: 'Scaffold (§7.1)', fields: [['cols', 'columns'], ['rows', 'rows'], ['cellW', 'cell width px'], ['cellH', 'cell height px'], ['gap', 'gap g px'], ['big', 'big blocks'], ['drop', 'share of cells dropped']] },
  { title: 'Shapes (§2, §7.2)', open: true, fields: [...CLASSES.map((c) => [`weights.${c}`, `weight ${c}`]), ...CLASSES.map((c) => [`maxOf.${c}`, `most ${c}`]), ['starCells', 'cells a star needs'], ['homophily', 'homophily h (−1..1)']] },
  { title: 'Directions (§1, §3)', fields: [['m', 'alphabet m', [4, 8]], ['discPorts', 'disc doors q', [4, 8]], ['portStep', 'door sampling px']] },
  {
    title: 'Corridors (§4, §5.3)',
    fields: [['width', 'width w px'], ['clear', 'clearance σ px'], ['exit', 'exit stub ℓs px'], ['seg', 'least segment px'], ['minLen', 'ℓmin px'], ['maxBends', 'most bends', [0, 1, 2]], ['pI', 'weight I'], ['pL', 'weight L'], ['pZ', 'weight Z/U'], ['pT', 'weight T'], ['pX', 'weight X'], ['costBend', 'cost per bend'], ['costLen', 'cost per px'], ['cost45', 'cost per 45° turn'], ['costFacing', 'bonus facing doors']],
  },
  { title: 'Mixing matrix M (§7.3.1)', fields: CLASSES.flatMap((a, i) => CLASSES.slice(i).map((b) => [`mix.${mixKey(a, b)}`, `${a} ↔ ${b}`])) },
  { title: 'Graph (§6, §7.3)', open: true, fields: [['proximity', 'candidate pairs', ['rng', 'gabriel', 'near']], ['near', 'near: max px'], ['delta', 'density δ (0..1)']] },
  { title: 'Roles (V5)', fields: [['stairsDoors', 'stairs: most doors d_s'], ['minHops', 'start→stairs least hops L*'], ['ambush', 'ambush rooms']] },
  { title: 'Leaf chambers (§7.6)', fields: [['leaves', 'how many'], ...CLASSES.map((c) => [`leafClasses.${c}`, `weight ${c}`]), ['leafRoles.trap', 'role trap'], ['leafRoles.prize', 'role prize'], ['leafRoles.empty', 'role empty']] },
]

const getPath = (o, path) => path.split('.').reduce((x, k) => x[k], o)
const setPath = (o, path, v) => {
  const keys = path.split('.')
  const last = keys.pop()
  keys.reduce((x, k) => x[k], o)[last] = v
}

const form = h('div')
const buildForm = () => {
  form.replaceChildren()
  for (const g of GROUPS) {
    const box = h('details', {}, h('summary', {}, g.title))
    box.open = !!g.open
    for (const [path, label, options] of g.fields) {
      const v = getPath(state.P, path)
      let input
      if (options) {
        input = h('select', {}, ...options.map((o) => h('option', { value: String(o) }, String(o))))
        input.value = String(v)
        input.addEventListener('change', () => {
          setPath(state.P, path, typeof v === 'number' ? Number(input.value) : input.value)
          regenerate()
        })
      } else {
        input = h('input', { type: 'number', step: 'any', value: String(v) })
        input.addEventListener('change', () => {
          const n = Number(input.value)
          if (!Number.isFinite(n)) return
          setPath(state.P, path, n)
          regenerate()
        })
      }
      box.append(h('label', { class: 'field' }, h('span', { title: path }, label), input))
    }
    form.append(box)
  }
}

/* ---------------- Drawing ---------------- */

const canvas = h('canvas')
const ctx = canvas.getContext('2d')

const pathOf = (s) => {
  ctx.beginPath()
  if (s.kind === 'disc') ctx.arc(s.c[0], s.c[1], s.r, 0, Math.PI * 2)
  else s.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
  ctx.closePath()
}

const draw = () => {
  const dpr = window.devicePixelRatio || 1
  const W = canvas.clientWidth
  const H = canvas.clientHeight
  canvas.width = Math.round(W * dpr)
  canvas.height = Math.round(H * dpr)
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, W, H)
  if (!current) return
  const d = current.dungeon
  const P = d.params
  const worldW = P.cols * P.cellW
  const worldH = P.rows * P.cellH
  const fit = Math.min((W - 40) / worldW, (H - 40) / worldH)
  const k = fit * view.zoom
  ctx.setTransform(dpr * k, 0, 0, dpr * k, dpr * (W / 2 + view.x - (worldW / 2) * k), dpr * (H / 2 + view.y - (worldH / 2) * k))
  const px = 1 / k

  if (overlays.cells) {
    ctx.strokeStyle = '#18222e'
    ctx.lineWidth = px
    for (let c = 0; c <= P.cols; c++) (ctx.beginPath(), ctx.moveTo(c * P.cellW, 0), ctx.lineTo(c * P.cellW, worldH), ctx.stroke())
    for (let r = 0; r <= P.rows; r++) (ctx.beginPath(), ctx.moveTo(0, r * P.cellH), ctx.lineTo(worldW, r * P.cellH), ctx.stroke())
  }
  if (overlays.pairs) {
    ctx.strokeStyle = 'rgba(120,160,220,0.5)'
    ctx.setLineDash([6 * px, 6 * px])
    ctx.lineWidth = px
    for (const [a, b] of d.pairs) {
      ctx.beginPath()
      ctx.moveTo(...d.rooms[a].center)
      ctx.lineTo(...d.rooms[b].center)
      ctx.stroke()
    }
    ctx.setLineDash([])
  }
  if (overlays.candidates) {
    ctx.strokeStyle = 'rgba(80,170,255,0.35)'
    ctx.lineWidth = 2 * px
    for (const c of d.candidates) {
      ctx.beginPath()
      c.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
      ctx.stroke()
    }
  }
  ctx.fillStyle = '#a89068'
  for (const e of d.edges) for (const s of e.region) (pathOf(s), ctx.fill())
  ctx.strokeStyle = 'rgba(40,30,20,0.6)'
  ctx.lineWidth = px
  for (const e of d.edges) {
    ctx.beginPath()
    e.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
    ctx.stroke()
  }
  for (const r of d.rooms) {
    if (!r.alive) continue
    pathOf(r.shape)
    ctx.fillStyle = COLORS[r.cls] ?? '#555'
    ctx.globalAlpha = r.leaf ? 0.6 : 0.9
    ctx.fill()
    ctx.globalAlpha = 1
    const role = ROLE[r.role]
    ctx.strokeStyle = role ?? '#0b1118'
    ctx.lineWidth = (role ? 4 : 1.5) * px
    ctx.stroke()
  }
  for (const r of d.rooms) {
    if (r.alive) continue
    pathOf(r.shape)
    ctx.strokeStyle = '#f55'
    ctx.setLineDash([4 * px, 4 * px])
    ctx.lineWidth = px
    ctx.stroke()
    ctx.setLineDash([])
  }
  if (overlays.ports) {
    ctx.fillStyle = 'rgba(255,255,255,0.35)'
    for (const r of d.rooms) for (const p of r.ports) ctx.fillRect(p.p[0] - 1.5 * px, p.p[1] - 1.5 * px, 3 * px, 3 * px)
  }
  ctx.strokeStyle = '#fff'
  ctx.lineWidth = 2 * px
  for (const r of d.rooms)
    for (const p of r.used) {
      const u = DIRS[p.k]
      ctx.beginPath()
      ctx.moveTo(p.p[0], p.p[1])
      ctx.lineTo(p.p[0] + u[0] * 14, p.p[1] + u[1] * 14)
      ctx.stroke()
    }
  for (const n of d.nodes) {
    if (n.kind !== 'junction') continue
    ctx.beginPath()
    ctx.arc(n.p[0], n.p[1], 6, 0, Math.PI * 2)
    ctx.fillStyle = '#fff'
    ctx.fill()
  }
  if (overlays.labels) {
    ctx.font = `${12 * px}px monospace`
    ctx.textAlign = 'center'
    for (const r of d.rooms) {
      if (!r.alive) continue
      const text = `#${r.id} ${r.cls}${r.role !== 'plain' ? ` ${r.role}` : ''} d${r.used.length}`
      ctx.lineWidth = 3 * px
      ctx.strokeStyle = '#000'
      ctx.strokeText(text, r.center[0], r.center[1] + 4 * px)
      ctx.fillStyle = '#fff'
      ctx.fillText(text, r.center[0], r.center[1] + 4 * px)
    }
  }
}

/* ---------------- Panels ---------------- */

const right = h('section')
const renderPanel = () => {
  right.replaceChildren()
  const r = current
  if (!r) return
  const m = r.metrics
  right.append(
    h('h3', {}, 'Validity (§5.2)'),
    r.violations.length === 0
      ? h('div', { class: 'ok' }, `valid ✓ (${r.draws} draw${r.draws > 1 ? 's' : ''}, ${r.ms.toFixed(1)} ms in the module)`)
      : h('div', { class: 'bad' }, `${r.violations.length} violation(s) after ${r.draws} draws`, h('ul', {}, ...r.violations.slice(0, 30).map((v) => h('li', {}, v)))),
  )
  const rows = [
    ['rooms (+ leaf chambers)', `${m.rooms} + ${m.leafChambers}`],
    ['junctions (T/X)', String(m.junctions)],
    ['corridor branches (edges of H)', String(m.edges)],
    ['cycles β (target / max)', `${m.beta} (${m.betaTarget} / ${m.betaMax})`],
    ['density δ = β/β_max', m.delta.toFixed(2)],
    ['mean doors per room', m.meanDegree.toFixed(2)],
    ['leaves (one door)', String(m.leaves)],
    ['bridges (share)', `${m.bridges} (${(m.bridgeShare * 100).toFixed(0)}%)`],
    ['articulation rooms', String(m.articulations)],
    ['hops start → stairs', m.hopsToStairs === null ? '—' : String(m.hopsToStairs)],
    ['diameter (hops)', String(m.diameter)],
    ['routes start → stairs (Menger)', String(m.routesToStairs)],
    ['λ₂ (algebraic connectivity)', m.lambda2.toFixed(3)],
  ]
  right.append(h('h3', {}, 'Graph (§6.2)'), h('table', {}, ...rows.map(([a, b]) => h('tr', {}, h('td', {}, a), h('td', {}, b)))))
  right.append(h('h3', {}, 'Shapes'), h('div', {}, Object.entries(m.byClass).map(([c, n]) => `${c} ${n}`).join(' · ')))
  if (r.dungeon.log.length > 0) right.append(h('h3', {}, 'Generator log'), h('ul', { class: 'dim' }, ...r.dungeon.log.map((l) => h('li', {}, l))))
  right.append(
    h('h3', {}, 'Legend'),
    h('div', { class: 'legend' }, ...Object.entries(COLORS).flatMap(([c, col]) => [h('span', { style: `background:${col}` }), `${c}  `])),
    h('div', { class: 'legend' }, ...Object.entries(ROLE).flatMap(([c, col]) => [h('span', { style: `background:${col}` }), `${c}  `])),
    h('div', { class: 'dim' }, 'white ticks: doors used (their direction) · white dots: junctions · dashed red: room left out'),
  )
}

/* ---------------- Controls ---------------- */

const regenerate = () => {
  try {
    current = JSON.parse(stage_json(state.seed >>> 0, JSON.stringify(state.P)))
  } catch (e) {
    current = null
    right.replaceChildren(h('div', { class: 'bad' }, String(e)))
    return
  }
  draw()
  renderPanel()
}

const levelButtons = [1, 2, 3, 4, 5].map((n) => {
  const b = h('button', { type: 'button' }, `Lv ${n}`)
  b.addEventListener('click', () => setLevel(n))
  return b
})
const setLevel = (n) => {
  state.level = n
  state.P = JSON.parse(default_params(n))
  levelButtons.forEach((b, i) => b.classList.toggle('on', i + 1 === n))
  buildForm()
  regenerate()
}
const seedIn = h('input', { type: 'number', value: '1', style: 'width:90px' })
seedIn.addEventListener('change', () => {
  state.seed = Math.max(0, Math.floor(Number(seedIn.value)) || 0)
  regenerate()
})
const newSeed = () => {
  state.seed = Math.floor(Math.random() * 1e6)
  seedIn.value = String(state.seed)
  regenerate()
}
const step = (d) => {
  state.seed = Math.max(0, state.seed + d)
  seedIn.value = String(state.seed)
  regenerate()
}
const button = (label, on) => {
  const b = h('button', { type: 'button' }, label)
  b.addEventListener('click', on)
  return b
}
const benchOut = h('div', { class: 'dim' })

const toggles = h('div', { class: 'row' }, ...Object.keys(overlays).map((key) => {
  const box = h('input', { type: 'checkbox' })
  box.checked = overlays[key]
  box.addEventListener('change', () => {
    overlays[key] = box.checked
    draw()
  })
  return h('label', {}, box, key)
}))

const left = h(
  'section',
  {},
  h('h3', {}, 'Dungeon model lab · Rust + WASM'),
  h('div', { class: 'dim' }, 'docs/dungeon-model.md · one generator, Λ per level'),
  h('div', { class: 'row' }, ...levelButtons),
  h('div', { class: 'row' }, 'seed', seedIn, button('◀', () => step(-1)), button('▶', () => step(1)), button('new seed (R)', newSeed)),
  h('div', { class: 'row' }, button('copy Λ (JSON)', () => void navigator.clipboard?.writeText(JSON.stringify({ level: state.level, seed: state.seed, params: state.P }, null, 2)).catch(() => {})), button('reset level', () => setLevel(state.level))),
  h('div', { class: 'row' }, button('benchmark: 50 stages of this level', () => {
    const r = JSON.parse(bench(state.level, 50))
    benchOut.textContent = `level ${state.level}: ${(r.ms / 50).toFixed(1)} ms per stage (${r.invalid} invalid of 50)`
  })),
  benchOut,
  h('h3', {}, 'Show'),
  toggles,
  h('h3', {}, 'Λ'),
  form,
)
const center = h('section', { id: 'view' }, canvas)
document.getElementById('app').replaceChildren(left, center, right)

canvas.addEventListener('wheel', (e) => {
  e.preventDefault()
  const rect = canvas.getBoundingClientRect()
  const sx = e.clientX - rect.left - rect.width / 2
  const sy = e.clientY - rect.top - rect.height / 2
  const dy = Math.max(-100, Math.min(100, e.deltaY * (e.deltaMode === 1 ? 33 : 1)))
  const zoom = Math.max(0.3, Math.min(20, view.zoom * Math.exp(-dy * 0.0012)))
  const ratio = zoom / view.zoom
  view.x = sx - (sx - view.x) * ratio
  view.y = sy - (sy - view.y) * ratio
  view.zoom = zoom
  draw()
}, { passive: false })
let drag = null
canvas.addEventListener('pointerdown', (e) => {
  drag = { x: e.clientX, y: e.clientY }
  canvas.setPointerCapture(e.pointerId)
})
canvas.addEventListener('pointermove', (e) => {
  if (!drag) return
  view.x += e.clientX - drag.x
  view.y += e.clientY - drag.y
  drag = { x: e.clientX, y: e.clientY }
  draw()
})
canvas.addEventListener('pointerup', () => (drag = null))
canvas.addEventListener('dblclick', () => {
  Object.assign(view, { zoom: 1, x: 0, y: 0 })
  draw()
})
window.addEventListener('resize', draw)
window.addEventListener('keydown', (e) => {
  if (e.target instanceof HTMLInputElement || e.target instanceof HTMLSelectElement) return
  if (e.key >= '1' && e.key <= '5') setLevel(Number(e.key))
  else if (e.key === 'r' || e.key === 'R') newSeed()
  else if (e.key === 'ArrowLeft') step(-1)
  else if (e.key === 'ArrowRight') step(1)
})

setLevel(1)

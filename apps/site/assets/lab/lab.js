// The browser lab. The server (Topcoat) renders every control, label and help text; this module binds to that markup and drives the
// WebAssembly generator, a pool of workers for batches, and the canvas. Text it builds while running comes from `data-i18n`.
import init, { default_params, stage_json, bench } from '/lab/pkg/tapiti.js'

const root = document.getElementById('lab')
const $ = (id) => document.getElementById(id)
const dict = JSON.parse(root.dataset.i18n)
const ISOLATES = /[⁦-⁩]/g // Fluent wraps values in invisible direction marks; a template needs none
const tr = (id, args = {}) => (dict[id] ?? id).replace(ISOLATES, '').replace(/\{(\w+)\}/g, (_, key) => args[key] ?? `{${key}}`)
const formatNumber = (n, digits = 1) => Number(n).toLocaleString('en-US', { maximumFractionDigits: digits })
const clone = (value) => JSON.parse(JSON.stringify(value))
const getPath = (object, path) => path.split('.').reduce((value, key) => value?.[key], object)
const setPath = (object, path, value) => {
  const keys = path.split('.')
  const last = keys.pop()
  keys.reduce((target, key) => target[key], object)[last] = value
}
const css = (name) => getComputedStyle(root).getPropertyValue(name).trim()
const remember = (key, value) => {
  try {
    if (value === undefined) return localStorage.getItem(key)
    localStorage.setItem(key, value)
  } catch {
    return null
  }
}

await init()

/* ---------------- Workers ---------------- */

// Each worker has its own single-threaded instance of the module. A stage is cheap (a few ms), so the parallelism that pays in the
// browser is across stages: a batch is cut into slices, one per worker, with no shared memory and no special headers on the page.
const createPool = (size = Math.max(1, Math.min(navigator.hardwareConcurrency || 4, 16))) => {
  const workers = Array.from({ length: size }, () => new Worker(root.dataset.worker, { type: 'module' }))
  let next = 0
  const pending = new Map()
  for (const w of workers) {
    w.onmessage = ({ data }) => {
      pending.get(data.id)?.(data)
      pending.delete(data.id)
    }
  }
  const batch = async (preset, from, count, use = size) => {
    const t0 = performance.now()
    const n = Math.max(1, Math.min(use, size, count))
    const slices = Array.from({ length: n }, (_, i) => [from + Math.floor((count * i) / n), Math.floor((count * (i + 1)) / n) - Math.floor((count * i) / n)])
    const answers = await Promise.all(
      slices.map(([f, c], i) => new Promise((resolve) => {
        const id = next++
        pending.set(id, resolve)
        workers[i].postMessage({ id, size: preset, from: f, count: c })
      })),
    )
    return { invalid: answers.reduce((a, r) => a + r.invalid, 0), ms: performance.now() - t0, workers: n }
  }
  return { batch }
}
let pool = null

/* ---------------- Constants ---------------- */

const CLASSES = ['rect', 'disc', 'oct', 'L', 'star']
const SIZE_NAMES = ['small', 'medium', 'large', 'huge']
const ROLES = ['start', 'stairs', 'ambush', 'boss', 'rest', 'key', 'switch', 'trap', 'prize', 'empty']
const className = (name) => dict[`lab_class_${name}`] ?? name
const roleName = (name) => dict[`lab_role_${name}`] ?? name
const DIRS = [[1, 0], [Math.SQRT1_2, Math.SQRT1_2], [0, 1], [-Math.SQRT1_2, Math.SQRT1_2], [-1, 0], [-Math.SQRT1_2, -Math.SQRT1_2], [0, -1], [Math.SQRT1_2, -Math.SQRT1_2]]

const state = { size: 1, seed: 1, P: JSON.parse(default_params(1)), defaults: JSON.parse(default_params(1)) }
const overlays = { cells: true, labels: true, ports: false, pairs: false, candidates: false }
const view = { zoom: 1, x: 0, y: 0 }
let current = null
let regenTimer = 0

/* ---------------- Help tooltips ---------------- */

const helpTooltip = $('help-tooltip')
let activeHelp = null

const hideHelp = () => {
  activeHelp?.setAttribute('aria-expanded', 'false')
  activeHelp = null
  helpTooltip.hidden = true
}
const showHelp = (button, pinned = false) => {
  if (activeHelp && activeHelp !== button) activeHelp.setAttribute('aria-expanded', 'false')
  activeHelp = button
  helpTooltip.textContent = button.dataset.help
  helpTooltip.hidden = false
  button.setAttribute('aria-expanded', pinned ? 'true' : 'false')
  const rect = button.getBoundingClientRect()
  const width = Math.min(320, window.innerWidth - 24)
  helpTooltip.style.width = `${width}px`
  helpTooltip.style.left = `${Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))}px`
  const below = rect.bottom + 8 + helpTooltip.offsetHeight < window.innerHeight
  helpTooltip.style.top = `${below ? rect.bottom + 8 : Math.max(12, rect.top - helpTooltip.offsetHeight - 8)}px`
}
const helpOf = (event) => event.target.closest?.('.help-button')
root.addEventListener('mouseover', (event) => { const b = helpOf(event); if (b) showHelp(b) })
root.addEventListener('focusin', (event) => { const b = helpOf(event); if (b) showHelp(b) })
root.addEventListener('mouseout', (event) => { const b = helpOf(event); if (b && activeHelp === b && document.activeElement !== b) hideHelp() })
root.addEventListener('focusout', (event) => { const b = helpOf(event); if (b && activeHelp === b && b.getAttribute('aria-expanded') !== 'true') hideHelp() })
root.addEventListener('click', (event) => {
  const b = helpOf(event)
  if (!b) return hideHelp()
  event.preventDefault()
  if (activeHelp === b && b.getAttribute('aria-expanded') === 'true') hideHelp()
  else showHelp(b, true)
})
window.addEventListener('scroll', hideHelp, true)

/* ---------------- Toast ---------------- */

const toast = $('lab-toast')
let toastTimer = 0
const showToast = (message) => {
  toast.textContent = message
  toast.hidden = false
  clearTimeout(toastTimer)
  toastTimer = setTimeout(() => { toast.hidden = true }, 2200)
}

/* ---------------- Parameter form ---------------- */

const form = $('parameter-groups')
const fields = [...form.querySelectorAll('[name]')]
const sizeDescription = $('size-description')
const changedStatus = $('changed-status')
const seedInput = $('seed-input')
const sizeButtons = [...root.querySelectorAll('.size-button')]

const syncForm = () => {
  for (const el of fields) {
    const value = getPath(state.P, el.name)
    if (el.dataset.kind === 'boolean') el.checked = !!value
    else el.value = String(value)
    el.removeAttribute('aria-invalid')
    el.parentElement.querySelector('.field-error').textContent = ''
  }
  const junction = state.P.junction
  $('junction-value').textContent = junction == null ? tr('lab_junction_inf') : formatNumber(junction, 2)
}

const updateChanged = () => {
  const n = fields.filter((el) => JSON.stringify(getPath(state.P, el.name)) !== JSON.stringify(getPath(state.defaults, el.name))).length
  changedStatus.textContent = n === 0 ? tr('lab_changed_none') : tr('lab_changed_some', { n })
  changedStatus.classList.toggle('changed', n > 0)
}

const updateSizeButtons = () => {
  sizeButtons.forEach((button, index) => {
    const selected = index + 1 === state.size
    button.classList.toggle('selected', selected)
    button.setAttribute('aria-pressed', String(selected))
  })
  sizeDescription.textContent = tr(`lab_size_${state.size}`)
}

const changed = () => {
  updateChanged()
  scheduleRegenerate()
}

form.addEventListener('change', (event) => {
  const el = event.target
  if (!el.name) return
  if (el.dataset.kind === 'boolean') setPath(state.P, el.name, el.checked)
  else if (el.dataset.kind === 'choice') setPath(state.P, el.name, typeof getPath(state.P, el.name) === 'number' ? Number(el.value) : el.value)
  else {
    const value = Number(el.value)
    const min = Number(el.min)
    const max = Number(el.max)
    const integer = el.dataset.integer !== undefined
    const error = el.parentElement.querySelector('.field-error')
    if (!el.value || !Number.isFinite(value) || value < min || value > max || (integer && !Number.isInteger(value))) {
      error.textContent = tr(integer ? 'lab_err_int' : 'lab_err_range', { min: formatNumber(min, 2), max: formatNumber(max, 2) })
      el.setAttribute('aria-invalid', 'true')
      return
    }
    error.textContent = ''
    el.removeAttribute('aria-invalid')
    setPath(state.P, el.name, value)
  }
  changed()
})

form.addEventListener('click', (event) => {
  const button = event.target.closest('.group-reset')
  if (!button) return
  for (const el of button.closest('details').querySelectorAll('[name]')) setPath(state.P, el.name, clone(getPath(state.defaults, el.name)))
  syncForm()
  changed()
})

function setSize(size) {
  state.size = size
  state.P = JSON.parse(default_params(size))
  state.defaults = clone(state.P)
  syncForm()
  updateSizeButtons()
  changed()
}

for (const button of sizeButtons) button.addEventListener('click', () => setSize(Number(button.dataset.size)))
$('reset-size').addEventListener('click', () => setSize(state.size))

/* ---------------- Seed, copy, overlays, experiments ---------------- */

const seedStep = (amount) => {
  state.seed = Math.max(0, Math.min(0xffffffff, state.seed + amount))
  seedInput.value = String(state.seed)
  scheduleRegenerate()
}
const newSeed = () => {
  state.seed = Math.floor(Math.random() * 1e6)
  seedInput.value = String(state.seed)
  scheduleRegenerate()
}
for (const button of root.querySelectorAll('[data-seed-step]')) button.addEventListener('click', () => seedStep(Number(button.dataset.seedStep)))
$('seed-random').addEventListener('click', newSeed)
seedInput.addEventListener('change', () => {
  const seed = Number(seedInput.value)
  if (!Number.isInteger(seed) || seed < 0 || seed > 0xffffffff) {
    seedInput.setAttribute('aria-invalid', 'true')
    seedInput.value = String(state.seed)
    showToast(tr('lab_seed_error'))
    return
  }
  seedInput.removeAttribute('aria-invalid')
  state.seed = seed
  scheduleRegenerate()
})

$('copy-params').addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(JSON.stringify({ size: SIZE_NAMES[state.size - 1], seed: state.seed, params: state.P }, null, 2))
    showToast(tr('lab_copied'))
  } catch {
    showToast(tr('lab_no_clipboard'))
  }
})

for (const input of root.querySelectorAll('[data-overlay]')) {
  overlays[input.dataset.overlay] = input.checked
  input.addEventListener('change', () => { overlays[input.dataset.overlay] = input.checked; draw() })
}

const EXPERIMENTS = {
  shapes: () => { for (const shape of CLASSES) state.P.weights[shape] = 1; state.P.m = 8 },
  cycles: () => { state.P.delta = 0.8 },
  junctions: () => { state.P.p_t = Math.max(state.P.p_t, 0.25); state.P.p_x = Math.max(state.P.p_x, 0.15) },
  rooms: () => { for (const shape of CLASSES) state.P.weights[shape] = shape === 'disc' ? 1 : 0 },
}
for (const button of root.querySelectorAll('[data-experiment]')) {
  button.addEventListener('click', () => {
    const kind = button.dataset.experiment
    EXPERIMENTS[kind]()
    showToast(tr(`lab_exp_${kind}_done`))
    syncForm()
    changed()
  })
}

/* ---------------- Benchmarks ---------------- */

const benchStatus = $('bench-status')
$('bench-40').addEventListener('click', () => {
  const result = JSON.parse(bench(state.size, 1, 40))
  benchStatus.textContent = tr('lab_perf_40_result', { ms: formatNumber(result.ms / 40, 2), invalid: result.invalid })
})
$('bench-800').addEventListener('click', async () => {
  benchStatus.textContent = tr('lab_perf_800_run')
  try {
    pool ??= createPool()
    const one = await pool.batch(state.size, 1, 800, 1)
    const all = await pool.batch(state.size, 1, 800)
    benchStatus.textContent = tr('lab_perf_800_result', { one: formatNumber(one.ms, 0), workers: all.workers, many: formatNumber(all.ms, 0), invalid: all.invalid })
  } catch (error) {
    benchStatus.textContent = tr('lab_perf_800_fail', { error })
  }
})

/* ---------------- Map drawing ---------------- */

const canvas = $('floor-map')
const ctx = canvas.getContext('2d')
const mapTooltip = $('map-tooltip')
const zoomLabel = $('zoom-label')
const viewSection = $('view')

const pathOf = (shape) => {
  ctx.beginPath()
  if (shape.kind === 'disc') ctx.arc(shape.c[0], shape.c[1], shape.r, 0, Math.PI * 2)
  else shape.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
  ctx.closePath()
}
const strokePolyline = (pts) => {
  ctx.beginPath()
  pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
  ctx.stroke()
}

const transformOf = () => {
  const W = canvas.clientWidth
  const H = canvas.clientHeight
  const P = current?.floor.params
  if (!P) return { W, H, k: 1, ox: 0, oy: 0 }
  const worldW = P.cols * P.cellW
  const worldH = P.rows * P.cellH
  const fit = Math.min(Math.max(1, W - 48) / worldW, Math.max(1, H - 48) / worldH)
  const k = fit * view.zoom
  return { W, H, k, ox: W / 2 + view.x - (worldW / 2) * k, oy: H / 2 + view.y - (worldH / 2) * k }
}

const draw = () => {
  const dpr = window.devicePixelRatio || 1
  const W = canvas.clientWidth
  const H = canvas.clientHeight
  if (!W || !H) return
  if (canvas.width !== Math.round(W * dpr) || canvas.height !== Math.round(H * dpr)) {
    canvas.width = Math.round(W * dpr)
    canvas.height = Math.round(H * dpr)
  }
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, W, H)
  zoomLabel.textContent = `${Math.round(view.zoom * 100)}%`
  if (!current) return
  const { k, ox, oy } = transformOf()
  ctx.setTransform(dpr * k, 0, 0, dpr * k, dpr * ox, dpr * oy)
  const px = 1 / k
  const d = current.floor
  const P = d.params
  const ink = css('--bone')
  const accent = css('--carrot')
  const corridor = css('--map-corridor')
  const surface = css('--stone-800')
  const colorOf = (cls) => css(`--map-${cls}`) || '#59677a'
  const roleColor = (role) => css(`--role-${role}`)

  if (overlays.cells) {
    ctx.strokeStyle = css('--stone-600')
    ctx.globalAlpha = 0.5
    ctx.lineWidth = px
    const gridX = d.gridX?.length === P.cols + 1 ? d.gridX : Array.from({ length: P.cols + 1 }, (_, i) => i * P.cellW)
    const gridY = d.gridY?.length === P.rows + 1 ? d.gridY : Array.from({ length: P.rows + 1 }, (_, i) => i * P.cellH)
    for (const x of gridX) strokePolyline([[x, gridY[0]], [x, gridY.at(-1)]])
    for (const y of gridY) strokePolyline([[gridX[0], y], [gridX.at(-1), y]])
    ctx.globalAlpha = 1
  }
  if (overlays.pairs) {
    ctx.strokeStyle = accent
    ctx.globalAlpha = 0.5
    ctx.setLineDash([7 * px, 6 * px])
    ctx.lineWidth = px
    for (const [a, b] of d.pairs) strokePolyline([d.rooms[a].center, d.rooms[b].center])
    ctx.setLineDash([])
    ctx.globalAlpha = 1
  }
  if (overlays.candidates) {
    ctx.strokeStyle = accent
    ctx.globalAlpha = 0.35
    ctx.lineWidth = 2 * px
    for (const candidate of d.candidates) strokePolyline(candidate.pts)
    ctx.globalAlpha = 1
  }
  if (d.edges.every((edge) => Array.isArray(edge.region))) {
    ctx.fillStyle = corridor
    for (const edge of d.edges) for (const shape of edge.region) (pathOf(shape), ctx.fill())
  } else {
    ctx.strokeStyle = corridor
    ctx.lineWidth = P.width
    ctx.lineCap = 'butt'
    ctx.lineJoin = 'round'
    for (const edge of d.edges) strokePolyline(edge.pts)
  }
  ctx.strokeStyle = ink
  ctx.globalAlpha = 0.55
  ctx.lineWidth = px
  ctx.lineCap = 'butt'
  ctx.lineJoin = 'round'
  for (const edge of d.edges) strokePolyline(edge.pts)
  ctx.globalAlpha = 1
  for (const room of d.rooms) {
    if (!room.alive) continue
    pathOf(room.shape)
    ctx.fillStyle = colorOf(room.cls)
    ctx.globalAlpha = room.leaf ? 0.62 : 0.9
    ctx.fill()
    ctx.globalAlpha = 1
    const role = roleColor(room.role)
    ctx.strokeStyle = role || ink
    ctx.lineWidth = (role ? 3.5 : 1.5) * px
    ctx.stroke()
  }
  for (const room of d.rooms) {
    if (room.alive) continue
    pathOf(room.shape)
    ctx.strokeStyle = css('--danger')
    ctx.setLineDash([4 * px, 4 * px])
    ctx.lineWidth = px
    ctx.stroke()
    ctx.setLineDash([])
  }
  if (overlays.ports) {
    ctx.fillStyle = ink
    ctx.globalAlpha = 0.35
    for (const room of d.rooms) for (const port of room.ports) ctx.fillRect(port.p[0] - 1.5 * px, port.p[1] - 1.5 * px, 3 * px, 3 * px)
    ctx.globalAlpha = 1
  }
  ctx.strokeStyle = ink
  ctx.lineWidth = 2 * px
  for (const room of d.rooms) for (const port of room.used) {
    const direction = DIRS[port.k]
    strokePolyline([port.p, [port.p[0] + direction[0] * 14, port.p[1] + direction[1] * 14]])
  }
  for (const node of d.nodes) {
    if (node.kind !== 'junction') continue
    ctx.beginPath()
    ctx.arc(node.p[0], node.p[1], 5, 0, Math.PI * 2)
    ctx.fillStyle = surface
    ctx.fill()
    ctx.strokeStyle = ink
    ctx.lineWidth = px
    ctx.stroke()
  }
  if (overlays.labels) {
    ctx.font = `${12 * px}px ${css('--font-body') || 'system-ui, sans-serif'}`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'alphabetic'
    const text = css('--bone')
    const halo = css('--abyss')
    for (const room of d.rooms) {
      if (!room.alive) continue
      const label = `#${room.id} ${className(room.cls)}${room.role !== 'plain' ? ` · ${roleName(room.role)}` : ''}`
      ctx.lineWidth = 2.2 * px
      ctx.strokeStyle = halo
      ctx.strokeText(label, room.center[0], room.center[1] + 4 * px)
      ctx.fillStyle = text
      ctx.fillText(label, room.center[0], room.center[1] + 4 * px)
    }
  }
}

/* ---------------- Details while exploring the map ---------------- */

const containsPoint = (shape, point) => {
  if (shape.kind === 'disc') return Math.hypot(point[0] - shape.c[0], point[1] - shape.c[1]) <= shape.r
  let inside = false
  const pts = shape.pts
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i]
    const [xj, yj] = pts[j]
    if ((yi > point[1]) !== (yj > point[1]) && point[0] < ((xj - xi) * (point[1] - yi)) / (yj - yi) + xi) inside = !inside
  }
  return inside
}

const distanceToSegment = (point, a, b) => {
  const dx = b[0] - a[0]
  const dy = b[1] - a[1]
  const length2 = dx * dx + dy * dy
  const t = length2 ? Math.max(0, Math.min(1, ((point[0] - a[0]) * dx + (point[1] - a[1]) * dy) / length2)) : 0
  return Math.hypot(point[0] - (a[0] + t * dx), point[1] - (a[1] + t * dy))
}

const nodeLabel = (floor, nodeId) => {
  const node = floor.nodes[nodeId]
  if (node?.kind === 'junction') return tr('lab_node_junction')
  const room = floor.rooms[node?.room]
  return room ? tr('lab_node_room', { id: room.id }) : tr('lab_node_point')
}

const mapObjectAt = (point, scale) => {
  const d = current.floor
  for (const room of [...d.rooms].reverse()) {
    if (!room.alive || !containsPoint(room.shape, point)) continue
    const role = room.leaf ? tr('lab_leaf_role', { role: roleName(room.role) || roleName('empty') }) : roleName(room.role)
    const lines = [tr('lab_tip_room', { role, id: room.id, class: className(room.cls), area: formatNumber(room.area, 0), used: room.used.length, possible: room.ports.length })]
    return lines.join('\n')
  }
  const junction = d.nodes.find((node) => node.kind === 'junction' && Math.hypot(node.p[0] - point[0], node.p[1] - point[1]) <= Math.max(6, 8 / scale))
  if (junction) {
    const id = d.nodes.indexOf(junction)
    const degree = d.edges.filter((edge) => edge.u === id || edge.v === id).length
    return tr('lab_tip_junction', { degree, x: formatNumber(junction.p[0], 0), y: formatNumber(junction.p[1], 0) })
  }
  const threshold = d.params.width / 2 + 5 / scale
  for (const edge of d.edges) {
    for (let i = 1; i < edge.pts.length; i++) {
      if (distanceToSegment(point, edge.pts[i - 1], edge.pts[i]) > threshold) continue
      const length = edge.pts.slice(1).reduce((sum, p, j) => sum + Math.hypot(p[0] - edge.pts[j][0], p[1] - edge.pts[j][1]), 0)
      const lines = [tr('lab_tip_corridor', { id: edge.id, from: nodeLabel(d, edge.u), to: nodeLabel(d, edge.v), n: edge.pts.length - 1, length: formatNumber(length, 0) })]
      return lines.join('\n')
    }
  }
  return ''
}

const hideMapTooltip = () => { mapTooltip.hidden = true }
const showMapTooltip = (event) => {
  if (!current) return
  const rect = canvas.getBoundingClientRect()
  const { k, ox, oy } = transformOf()
  const content = mapObjectAt([(event.clientX - rect.left - ox) / k, (event.clientY - rect.top - oy) / k], k)
  if (!content) return hideMapTooltip()
  mapTooltip.textContent = content
  mapTooltip.hidden = false
  const viewRect = viewSection.getBoundingClientRect()
  mapTooltip.style.left = `${Math.max(10, Math.min(event.clientX - viewRect.left + 14, viewRect.width - 250))}px`
  mapTooltip.style.top = `${Math.max(58, Math.min(event.clientY - viewRect.top + 14, viewRect.height - 100))}px`
}

/* ---------------- Results panel ---------------- */

const results = $('results')
const el = (tag, attrs = {}, ...kids) => {
  const node = document.createElement(tag)
  for (const [key, value] of Object.entries(attrs)) node.setAttribute(key, value)
  node.append(...kids)
  return node
}
const helpButton = (text) => el('button', { type: 'button', class: 'help-button', 'data-help': text, 'aria-label': tr('lab_help_prefix', { text }), 'aria-expanded': 'false', 'aria-describedby': 'help-tooltip' }, '?')
const swatch = (color) => { const i = el('i', { 'aria-hidden': 'true' }); i.style.background = color; return i }

const translate = (message, table) => {
  for (const [pattern, id] of table) {
    const match = typeof pattern === 'string' ? (message === pattern ? [] : null) : message.match(pattern)
    if (match) return tr(id, { a: match[1], b: match[2], c: match[3] })
  }
  return message
}
const VIOLATIONS = [
  [/^V1 room (\d+) not reached from the start$/, 'lab_v_v1'],
  [/^V2 rooms (\d+) and (\d+) closer than 2g$/, 'lab_v_v2_rooms'],
  [/^V4 corridor (.+) reaches (\d+) room\(s\): every corridor joins rooms$/, 'lab_v_v4_reach'],
  [/^V4 rooms (\d+) and (\d+): corridor (.+) px < ℓmin$/, 'lab_v_v4_short'],
  [/^V2 rooms (\d+) and (\d+) joined but closer than ℓmin in a straight line$/, 'lab_v_v2_straight'],
  [/^V2 corridor (\d+) runs into room (\d+) past its door$/, 'lab_v_v2_past_door'],
  [/^V2 corridor (\d+) closer than σ to room (\d+)$/, 'lab_v_v2_corr_room'],
  [/^V2 corridors (\d+) and (\d+) closer than σ$/, 'lab_v_v2_corr_corr'],
  [/^V3 room (\d+): a door at 45° in a retilinear stage$/, 'lab_v_v3_diag'],
  [/^V3 room (\d+): a door's exit band runs back into the room$/, 'lab_v_v3_back'],
  [/^V3 room (\d+): a door off the circle$/, 'lab_v_v3_circle'],
  [/^V3 room (\d+): doors (\d+) and (\d+) closer than σ$/, 'lab_v_v3_doors'],
  [/^V4 corridor (\d+): a segment off the alphabet$/, 'lab_v_v4_alphabet'],
  [/^V4 corridor (\d+): a (\d+)° turn$/, 'lab_v_v4_turn'],
  [/^V4 corridor (\d+): a segment of (.+) px$/, 'lab_v_v4_segment'],
  [/^V4 corridor (\d+): leaves a door at an angle$/, 'lab_v_v4_angle'],
  [/^V5 the stairs have (\d+) doors \(most (\d+)\)$/, 'lab_v_v5_doors'],
  [/^V5 the stairs are (.+) hops from the start \(least (\d+)\)$/, 'lab_v_v5_hops'],
  ['V5 start and stairs must be two rooms of the stage', 'lab_v_v5_distinct'],
  [/^T1 profile requires (\d+) side branches; found (\d+)$/, 'lab_v_t1'],
  [/^T2 profile requires branch depth (\d+); found (\d+)$/, 'lab_v_t2'],
  [/^T3 profile requires (\d+) independent routes to stairs; found (\d+)$/, 'lab_v_t3'],
  [/^T4 profile requires a hub degree of (\d+); found (\d+)$/, 'lab_v_t4'],
  [/^T5 profile requires a cycle of at least (\d+) branches; found (\d+)$/, 'lab_v_t5'],
]
const LOGS = [
  [/^tree stuck: candidates from (.+)$/i, 'lab_l_stuck_candidates'],
  [/^tree stuck: near (.+) px$/i, 'lab_l_stuck_near'],
  [/^(.+) room\(s\) left out: no valid corridor reaches them$/, 'lab_l_left_out'],
  [/^stairs only (.+) hops from the start \(wanted (.+)\)$/, 'lab_l_stairs'],
  [/^β\* = (.+) not reached: nothing more fits \(β = (.+)\)$/, 'lab_l_beta'],
  [/^block (\d+): (.+) does not fit, rect instead$/, 'lab_l_block'],
  [/^(.+) of (.+) leaf chambers fit$/, 'lab_l_leaves'],
  [/^topology profile: could not reach (\d+) independent routes to stairs \(λ = (\d+)\)$/, 'lab_l_routes'],
  [/^topology profile: no valid junction can reach degree (\d+)$/, 'lab_l_hub'],
  [/^topology profile: no valid cycle of at least (\d+) branches fits$/, 'lab_l_cycle'],
  [/^topology profile: added (\d+) of (\d+) required side branches$/, 'lab_l_branches'],
  [/^layout jitter limited to (.+) \(requested (.+)\) to preserve minimum room size$/, 'lab_l_jitter'],
  [/shape class caps cannot fit/i, 'lab_l_caps'],
]

const METRIC_ROWS = (m) => ({
  lab_g_size: [
    ['rooms', `${m.rooms}`], ['leaf_chambers', `${m.leafChambers}`], ['junctions', `${m.junctions}`], ['edges', `${m.edges}`],
  ],
  lab_g_cycles: [
    ['beta', `${m.beta}`], ['beta_target', `${m.betaTarget}`], ['beta_max', `${m.betaMax}`], ['delta', formatNumber(m.delta, 2)],
  ],
  lab_g_paths: [
    ['mean_degree', formatNumber(m.meanDegree, 2)], ['leaves', `${m.leaves}`],
    ['bridges', `${m.bridges} · ${formatNumber(m.bridgeShare * 100, 0)}%`], ['articulations', `${m.articulations}`],
    ['hops', m.hopsToStairs == null ? '—' : formatNumber(m.hopsToStairs, 0)], ['diameter', formatNumber(m.diameter, 0)],
    ['routes', `${m.routesToStairs}`], ['girth', m.cycleGirth == null ? '—' : `${m.cycleGirth}`],
    ['side_branches', `${m.sideBranches}`], ['branch_depth', `${m.deepestBranch}`], ['max_degree', `${m.maxNodeDegree}`],
    ['geo_path', `${formatNumber(m.geometricPathToStairs, 0)} px`], ['lambda2', formatNumber(m.lambda2, 3)],
  ],
})

const renderResults = () => {
  results.replaceChildren()
  if (!current) return
  const { metrics: m, violations, draws, ms } = current
  const ok = violations.length === 0
  const status = el('div', { class: `validity-card ${ok ? 'valid' : 'invalid'}` },
    el('span', { class: 'status-icon', 'aria-hidden': 'true' }, ok ? '✓' : '!'),
    el('div', {},
      el('strong', {}, ok ? tr('lab_valid') : tr('lab_invalid', { n: violations.length })),
      el('p', {}, ok ? tr('lab_valid_detail', { draws, ms: formatNumber(ms, 1) }) : tr('lab_invalid_detail', { draws })),
      ...(ok ? [] : [el('ul', { class: 'violation-list' }, ...violations.slice(0, 8).map((line) => el('li', {}, translate(line, VIOLATIONS))))]),
    ))
  results.append(el('div', { class: 'panel-header' }, el('p', { class: 'eyebrow' }, tr('lab_panel_eyebrow', { size: SIZE_NAMES[state.size - 1], seed: state.seed })), el('h2', {}, tr('lab_panel_title'))), status)

  const groups = METRIC_ROWS(m)
  Object.entries(groups).forEach(([title, rows], index) => {
    const table = el('table', { class: 'metric-table' }, el('tbody', {}, ...rows.map(([id, value]) =>
      el('tr', {}, el('th', { scope: 'row' }, el('div', { class: 'metric-label' }, tr(`lab_m_${id}`), helpButton(tr(`lab_m_${id}_help`)))), el('td', {}, value)))))
    const details = el('details', { class: 'result-group' }, el('summary', {}, tr(title)), table)
    details.open = index < 2
    results.append(details)
  })

  const chips = Object.entries(m.byClass).map(([name, count]) => el('span', { class: 'shape-chip' }, swatch(`var(--map-${name})`), `${className(name)}: ${count}`))
  results.append(el('section', { class: 'result-section' }, el('h3', {}, tr('lab_shapes_title')), el('div', { class: 'chip-list' }, ...chips)))

  if (current.floor.log.length) {
    results.append(el('details', { class: 'result-group' }, el('summary', {}, tr('lab_log_title')), el('ul', { class: 'log-list' }, ...current.floor.log.map((line) => el('li', {}, translate(line, LOGS))))))
  }

  const legend = (names, color) => names.map((name) => el('span', { class: 'legend-item' }, swatch(color(name)), name === 'plain' ? '' : (color === shapeColor ? className(name) : roleName(name))))
  const shapeColor = (name) => `var(--map-${name})`
  const roleColor = (name) => `var(--role-${name})`
  results.append(el('details', { class: 'result-group' }, el('summary', {}, tr('lab_legend_title')),
    el('div', { class: 'legend-block' },
      el('strong', {}, tr('lab_legend_shapes')), el('div', { class: 'legend-list' }, ...legend(CLASSES, shapeColor)),
      el('strong', {}, tr('lab_legend_roles')), el('div', { class: 'legend-list' }, ...legend(ROLES, roleColor)))))
}

/* ---------------- Generation ---------------- */

const regenerate = () => {
  try {
    current = JSON.parse(stage_json(state.seed >>> 0, JSON.stringify(state.P)))
    hideHelp()
    draw()
    renderResults()
    if (view3d.active) mount3d()
  } catch (error) {
    current = null
    results.replaceChildren(el('div', { class: 'validity-card invalid' }, el('div', {}, el('strong', {}, tr('lab_gen_failed')), el('p', {}, String(error)))))
    draw()
  }
}
function scheduleRegenerate() {
  clearTimeout(regenTimer)
  updateSizeButtons()
  regenTimer = setTimeout(regenerate, 80)
}

/* ---------------- Pan, zoom, keys ---------------- */

const zoomBy = (factor) => {
  const next = Math.max(0.3, Math.min(20, view.zoom * factor))
  const ratio = next / view.zoom
  view.x *= ratio
  view.y *= ratio
  view.zoom = next
  draw()
}
const resetView = () => { view.zoom = 1; view.x = 0; view.y = 0; draw() }
$('zoom-out').addEventListener('click', () => zoomBy(0.8))
$('zoom-in').addEventListener('click', () => zoomBy(1.25))
$('zoom-fit').addEventListener('click', resetView)

let drag = null
canvas.addEventListener('wheel', (event) => {
  event.preventDefault()
  const rect = canvas.getBoundingClientRect()
  const sx = event.clientX - rect.left - rect.width / 2
  const sy = event.clientY - rect.top - rect.height / 2
  const dy = Math.max(-100, Math.min(100, event.deltaY * (event.deltaMode === 1 ? 33 : 1)))
  const next = Math.max(0.3, Math.min(20, view.zoom * Math.exp(-dy * 0.0012)))
  const ratio = next / view.zoom
  view.x = sx - (sx - view.x) * ratio
  view.y = sy - (sy - view.y) * ratio
  view.zoom = next
  draw()
}, { passive: false })
canvas.addEventListener('pointerdown', (event) => {
  hideMapTooltip()
  drag = { x: event.clientX, y: event.clientY, moved: false }
  canvas.classList.add('dragging')
  canvas.setPointerCapture(event.pointerId)
})
canvas.addEventListener('pointermove', (event) => {
  if (!drag) return showMapTooltip(event)
  const dx = event.clientX - drag.x
  const dy = event.clientY - drag.y
  if (Math.abs(dx) + Math.abs(dy) > 2) drag.moved = true
  if (drag.moved) {
    view.x += dx
    view.y += dy
    drag.x = event.clientX
    drag.y = event.clientY
    hideMapTooltip()
    draw()
  }
})
canvas.addEventListener('pointerup', (event) => {
  const wasDrag = drag?.moved
  drag = null
  canvas.classList.remove('dragging')
  if (!wasDrag) showMapTooltip(event)
})
canvas.addEventListener('pointercancel', () => { drag = null; canvas.classList.remove('dragging'); hideMapTooltip() })
canvas.addEventListener('pointerleave', (event) => { if (!drag && event.pointerType !== 'touch') hideMapTooltip() })
canvas.addEventListener('dblclick', resetView)

new ResizeObserver(draw).observe(viewSection)
window.addEventListener('keydown', (event) => {
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement || event.target?.isContentEditable) return
  // The 3D view uses the arrow keys itself, to move.
  if (event.target === canvas3d) return
  if (event.altKey || event.ctrlKey || event.metaKey) return
  if (event.key >= '1' && event.key <= '4') setSize(Number(event.key))
  else if (event.key === 'r' || event.key === 'R') newSeed()
  else if (event.key === 'ArrowLeft') seedStep(-1)
  else if (event.key === 'ArrowRight') seedStep(1)
})

/* ---------------- The reading drawer and the tabs ---------------- */

const readingToggle = $('reading-toggle')
const results3 = $('results')
const setReading = (open, { focus = true } = {}) => {
  root.dataset.reading = open ? 'open' : ''
  readingToggle.setAttribute('aria-expanded', String(open))
  remember('tapiti.lab.reading', open ? '1' : '0')
  if (!focus) return
  if (open) { results3.tabIndex = -1; results3.focus() } else readingToggle.focus()
}
readingToggle.addEventListener('click', () => setReading(root.dataset.reading !== 'open'))
root.addEventListener('keydown', (event) => {
  if (event.key === 'Escape' && root.dataset.reading === 'open') setReading(false)
})
if (remember('tapiti.lab.reading') === '1') setReading(true, { focus: false })

const tabs = [...root.querySelectorAll('.lab-tabs [role=tab]')]
const selectTab = (tab, { focus = false } = {}) => {
  root.dataset.tab = tab.dataset.tab
  for (const other of tabs) { other.setAttribute('aria-selected', String(other === tab)); other.tabIndex = other === tab ? 0 : -1 }
  if (focus) tab.focus()
}
tabs.forEach((tab, i) => {
  tab.addEventListener('click', () => selectTab(tab))
  tab.addEventListener('keydown', (event) => {
    const next = { ArrowRight: 1, ArrowLeft: -1 }[event.key]
    if (next) { event.preventDefault(); selectTab(tabs[(i + next + tabs.length) % tabs.length], { focus: true }) }
  })
})
selectTab(tabs[0])

/* ---------------- Plan and 3D ---------------- */

const canvas3d = $('floor-3d')
const viewSwitch = root.querySelector('.view-switch')
const instructions = $('map-instructions')
const view3d = { active: false, modules: null, handle: null }
const hasWebGL2 = () => { try { return !!document.createElement('canvas').getContext('webgl2') } catch { return false } }
if (hasWebGL2()) viewSwitch.hidden = false

const setView = (to3d) => {
  view3d.active = to3d
  canvas.hidden = to3d
  canvas3d.hidden = !to3d
  hideMapTooltip()
  for (const id of ['zoom-out', 'zoom-in', 'zoom-fit']) $(id).hidden = to3d
  zoomLabel.hidden = to3d
  $('view-plan').setAttribute('aria-pressed', String(!to3d))
  $('view-3d').setAttribute('aria-pressed', String(to3d))
  canvas3d.setAttribute('aria-describedby', 'map-instructions')
  instructions.textContent = tr(to3d ? 'lab_3d_instructions' : 'lab_map_instructions')
}
let mounting = 0
async function mount3d() {
  if (!current) return
  const ticket = ++mounting
  try {
    view3d.modules ??= await Promise.all([import(root.dataset.three), import(root.dataset.build), import(root.dataset.scene)])
    if (ticket !== mounting || !view3d.active) return
    const [THREE, build, scene] = view3d.modules
    view3d.handle?.dispose({ keepContext: true })
    view3d.handle = scene.mount({ THREE, canvas: canvas3d, floor: current.floor, scene: build.build(current.floor), mode: 'orbit' })
  } catch (error) {
    console.warn('lab: the 3D view did not start', error)
    leave3d()
  }
}
function leave3d() {
  mounting++
  view3d.handle?.dispose()
  view3d.handle = null
  setView(false)
  draw()
}
$('view-3d').addEventListener('click', () => { if (view3d.active) return; setView(true); mount3d() })
$('view-plan').addEventListener('click', () => { if (view3d.active) leave3d() })

syncForm()
updateSizeButtons()
updateChanged()
regenerate()

import init, { default_params, stage_json, bench } from './pkg/tapiti.js'
import { createPool } from './pool.js'

await init()
const pool = createPool()

const CLASSES = ['rect', 'disc', 'oct', 'L', 'star']
const CLASS_LABELS = { rect: 'rectangular', disc: 'circular', oct: 'octagonal', L: 'L-shaped', star: 'truncated star' }
const COLORS = { rect: '#496c9a', disc: '#29a89a', oct: '#9b78d1', L: '#ce843c', star: '#d0ae38' }
const ROLE_COLORS = { start: '#62d99a', stairs: '#dc78dc', ambush: '#fb7777', boss: '#ff827d', rest: '#6fc8e8', key: '#ffca58', switch: '#7bb9ff', trap: '#f26c6c', prize: '#f3c45d', empty: '#a7b1c0' }
const ROLE_LABELS = { start: 'entrance', stairs: 'stairs', ambush: 'ambush', boss: 'boss', rest: 'rest', key: 'key', switch: 'switch', trap: 'trap', prize: 'prize', empty: 'empty', plain: 'plain' }
const DIRS = [[1, 0], [Math.SQRT1_2, Math.SQRT1_2], [0, 1], [-Math.SQRT1_2, Math.SQRT1_2], [-1, 0], [-Math.SQRT1_2, -Math.SQRT1_2], [0, -1], [Math.SQRT1_2, -Math.SQRT1_2]]
const LEVEL_INFO = [
  'Introduction: rectangular rooms and few extra connections.',
  'Adds L-shaped rooms and the first junctions.',
  'Enlarges the map and unlocks diagonal paths and circular rooms.',
  'Combines more shapes, junctions and stairs doors.',
  'Richest setup: more rooms, shapes and connection alternatives.',
]

const state = { level: 1, seed: 1, P: JSON.parse(default_params(1)), defaults: JSON.parse(default_params(1)) }
const overlays = { cells: true, labels: true, ports: false, pairs: false, candidates: false }
const view = { zoom: 1, x: 0, y: 0 }
let current = null
let regenTimer = 0
let activeHelp = null
let helpId = 0

const h = (tag, attrs = {}, ...kids) => {
  const el = document.createElement(tag)
  for (const [key, value] of Object.entries(attrs)) el.setAttribute(key, value)
  el.append(...kids)
  return el
}
const clone = (value) => JSON.parse(JSON.stringify(value))
const formatNumber = (n, digits = 1) => Number(n).toLocaleString('en-US', { maximumFractionDigits: digits })

/* ---------------- Accessible help ---------------- */

const helpTooltip = h('div', { id: 'help-tooltip', class: 'help-tooltip', role: 'tooltip', hidden: '' })
document.body.append(helpTooltip)

const hideHelp = () => {
  if (activeHelp) activeHelp.setAttribute('aria-expanded', 'false')
  activeHelp = null
  helpTooltip.hidden = true
  helpTooltip.classList.remove('visible')
}

const showHelp = (button, text, pinned = false) => {
  if (activeHelp && activeHelp !== button) activeHelp.setAttribute('aria-expanded', 'false')
  activeHelp = button
  helpTooltip.textContent = text
  helpTooltip.hidden = false
  helpTooltip.classList.add('visible')
  button.setAttribute('aria-expanded', pinned ? 'true' : 'false')
  const rect = button.getBoundingClientRect()
  const width = Math.min(320, window.innerWidth - 24)
  helpTooltip.style.width = `${width}px`
  const left = Math.max(12, Math.min(rect.left, window.innerWidth - width - 12))
  const top = rect.bottom + 8 + helpTooltip.offsetHeight < window.innerHeight ? rect.bottom + 8 : Math.max(12, rect.top - helpTooltip.offsetHeight - 8)
  helpTooltip.style.left = `${left}px`
  helpTooltip.style.top = `${top}px`
}

const help = (text) => {
  const button = h('button', { type: 'button', class: 'help-button', 'aria-label': `Help: ${text}`, 'aria-expanded': 'false', 'aria-describedby': 'help-tooltip' }, '?')
  button.addEventListener('mouseenter', () => showHelp(button, text))
  button.addEventListener('focus', () => showHelp(button, text))
  button.addEventListener('mouseleave', () => { if (activeHelp === button && document.activeElement !== button) hideHelp() })
  button.addEventListener('blur', () => { if (activeHelp === button && button.getAttribute('aria-expanded') !== 'true') hideHelp() })
  button.addEventListener('click', (event) => {
    event.preventDefault()
    event.stopPropagation()
    if (activeHelp === button && button.getAttribute('aria-expanded') === 'true') hideHelp()
    else showHelp(button, text, true)
  })
  return button
}

document.addEventListener('click', (event) => {
  if (!event.target.closest('.help-button')) hideHelp()
})
window.addEventListener('scroll', hideHelp, true)
window.addEventListener('resize', hideHelp)

/* ---------------- Parameters ---------------- */

const mixKey = (a, b) => (CLASSES.indexOf(a) <= CLASSES.indexOf(b) ? `${a}-${b}` : `${b}-${a}`)

const GROUPS = [
  {
    id: 'scaffold', title: 'Grid and size', open: false,
    intro: 'Defines the available space and how many blocks make up the floor.',
    fields: [['cols', 'Columns'], ['rows', 'Rows'], ['cellW', 'Cell width (px)'], ['cellH', 'Cell height (px)'], ['gap', 'Room inset g (px)'], ['big', 'Large blocks'], ['drop', 'Fraction of free cells'], ['layoutJitter', 'Grid irregularity'], ['rotateRooms', 'Rotate polygonal shapes']],
  },
  {
    id: 'shapes', title: 'Room shapes', open: true,
    intro: 'Higher weights make a shape more likely. The limits control the maximum number of each type.',
    fields: [...CLASSES.map((c) => [`weights.${c}`, `Weight · ${CLASS_LABELS[c]}`]), ...CLASSES.map((c) => [`maxOf.${c}`, `Max · ${CLASS_LABELS[c]}`]), ['starCells', 'Minimum cells for a star'], ['homophily', 'Preference for neighboring shapes']],
  },
  {
    id: 'directions', title: 'Directions and doors', open: false,
    intro: 'Controls the directions allowed for corridors and how doors are sampled.',
    fields: [['m', 'Direction alphabet'], ['discPorts', 'Doors per circular room'], ['portStep', 'Step between doors (px)']],
  },
  {
    id: 'corridors', title: 'Corridors and junctions', open: false,
    intro: 'Measurements are in pixels. Weights favor a kind of link; costs change the preference among valid paths.',
    fields: [['width', 'Width w (px)'], ['clear', 'Clearance σ (px)'], ['exit', 'Exit stretch (px)'], ['seg', 'Minimum segment (px)'], ['minLen', 'Minimum distance between rooms (px)'], ['maxBends', 'Maximum bends'], ['pI', 'Weight · straight I'], ['pL', 'Weight · one L bend'], ['pZ', 'Weight · two bends Z/U'], ['pW', 'Weight · W detour'], ['pT', 'Relative chance · T junction'], ['pX', 'Relative chance · X crossing'], ['costBend', 'Cost per bend'], ['costLen', 'Cost per pixel'], ['cost45', 'Cost per 45° bend'], ['costFacing', 'Bonus for facing doors']],
  },
  {
    id: 'mix', title: 'Shape compatibility', open: false,
    intro: 'Each value multiplies the chance of connecting the two room types. Zero forbids the connection; one leaves the chance unchanged.',
    fields: CLASSES.flatMap((a, i) => CLASSES.slice(i).map((b) => [`mix.${mixKey(a, b)}`, `${CLASS_LABELS[a]} ↔ ${CLASS_LABELS[b]}`])),
  },
  {
    id: 'graph', title: 'Connections and density', open: true,
    intro: 'Chooses which pairs of rooms can be considered and how many cycles the generator tries to create.',
    fields: [['proximity', 'How to choose pairs'], ['near', 'Maximum radius between centers (px)'], ['delta', 'Density δ']],
  },
  {
    id: 'topology', title: 'Topology profile', open: false,
    intro: 'Pick a connectivity pattern or tune its requirements. The generator tries to satisfy them without breaking the geometric rules.',
    fields: [['topology.profile', 'Profile'], ['topology.minBranches', 'Minimum side branches'], ['topology.minBranchDepth', 'Minimum branch depth'], ['topology.minExitRoutes', 'Independent routes to the stairs'], ['topology.minCycleLength', 'Minimum cycle length'], ['topology.minHubDegree', 'Minimum degree of a central point']],
  },
  {
    id: 'progression', title: 'Progression and pacing', open: false,
    intro: 'Add doors that require items or switches and spread roles along the main path.',
    fields: [['progression.keyLocks', 'Key locks'], ['progression.switchLocks', 'Switch locks'], ['pacing.enabled', 'Enable exploration pacing']],
  },
  {
    id: 'roles', title: 'Entrance, stairs and roles', open: false,
    intro: 'Controls the stairs doors, the minimum distance between entrance and stairs, and the number of ambushes.',
    fields: [['stairsDoors', 'Maximum doors on the stairs'], ['minHops', 'Minimum steps to the stairs'], ['ambush', 'Ambush rooms']],
  },
  {
    id: 'leaves', title: 'Isolated chambers', open: false,
    intro: 'Leaf chambers are extra rooms linked by a single corridor, like dead ends with a prize or a trap.',
    fields: [['leaves', 'Number of chambers'], ...CLASSES.map((c) => [`leafClasses.${c}`, `Weight · ${CLASS_LABELS[c]}`]), ['leafRoles.trap', 'Relative chance · trap'], ['leafRoles.prize', 'Relative chance · prize'], ['leafRoles.empty', 'Relative chance · empty']],
  },
]

const PARAM_HELP = {
  cols: 'How many columns of cells make up the grid. More columns raise the potential width of the floor.',
  rows: 'How many rows of cells make up the grid. More rows raise the potential height of the floor.',
  cellW: 'Width of each grid cell, in pixels. Large blocks can span several cells.',
  cellH: 'Height of each grid cell, in pixels. Large blocks can span several cells.',
  gap: 'Inset of each block before the room is drawn. Helps keep space between neighboring rooms.',
  big: 'Maximum number of large blocks the generator tries to place before filling the rest with smaller rooms.',
  drop: 'Fraction of small cells the generator tries to leave free for future leaf chambers. E.g. 0.2 means 20%.',
  layoutJitter: 'Shifts inner grid lines by up to this fraction of the cell width or height. The outer borders and the order of the lines are preserved; the generator lowers the value if there is not enough space for a minimum room.',
  rotateRooms: 'Allows polygonal shapes to rotate in steps of 90° (or 45° in octilinear mode), increasing the variety of orientations.',
  'weights.rect': 'Draw weight for rectangular rooms. It is relative to the weights of the other shapes; it does not need to sum to 1.',
  'weights.disc': 'Draw weight for circular rooms. It is relative to the weights of the other shapes; it does not need to sum to 1.',
  'weights.oct': 'Draw weight for octagonal rooms. It is relative to the weights of the other shapes; it does not need to sum to 1.',
  'weights.L': 'Draw weight for L-shaped rooms. It is relative to the weights of the other shapes; it does not need to sum to 1.',
  'weights.star': 'Draw weight for truncated stars. They can only appear in blocks with enough cells.',
  'maxOf.rect': 'Maximum number of rectangular rooms. If the limits sum to less than the total number of rooms, some limit may be exceeded.',
  'maxOf.disc': 'Maximum number of circular rooms. If the limits sum to less than the total number of rooms, some limit may be exceeded.',
  'maxOf.oct': 'Maximum number of octagonal rooms. If the limits sum to less than the total number of rooms, some limit may be exceeded.',
  'maxOf.L': 'Maximum number of L-shaped rooms. If the limits sum to less than the total number of rooms, some limit may be exceeded.',
  'maxOf.star': 'Maximum number of truncated stars. If the limits sum to less than the total number of rooms, some limit may be exceeded.',
  starCells: 'Minimum number of cells in the block to allow a truncated star.',
  homophily: 'Controls the influence of neighboring shapes: a positive value brings equal rooms together; negative favors different rooms; zero ignores neighbors.',
  m: '4 allows straight corridors at 90° angles. 8 also allows 45° diagonals.',
  discPorts: 'Number of possible directions on the edge of circular rooms. Directions outside the current alphabet are discarded.',
  portStep: 'Distance between door positions sampled on polygonal edges. A smaller step tries more positions, but raises the generation cost.',
  width: 'Physical width of each corridor.',
  clear: 'Minimum distance between corridors and rooms or corridors they do not connect to.',
  exit: 'Minimum length of the corridor when leaving a door before it can turn.',
  seg: 'Minimum length of each inner corridor segment.',
  minLen: 'Requires both the path and the straight-line distance between connected rooms to reach this length.',
  maxBends: 'Maximum number of bends in a link between two doors: 0 straight, 1 L, 2 Z/U. Values from 4 to 6 allow more winding W detours.',
  pI: 'Relative weight of a straight link, without bends.',
  pL: 'Relative weight of a link with one bend.',
  pZ: 'Relative weight of a link with two bends.',
  pW: 'Relative weight of a path with an extra W-shaped detour. It only yields alternatives when at least four bends are available.',
  pT: 'Relative weight for linking a room to an existing corridor, forming a T junction.',
  pX: 'Relative weight for crossing an existing corridor and forming an X junction.',
  costBend: 'Penalty added for each bend. A higher cost favors straighter paths.',
  costLen: 'Penalty multiplied by the path length, in pixels.',
  cost45: 'Additional penalty for each diagonal 45° bend.',
  costFacing: 'Discount when doors face each other and allow a straight link.',
  proximity: 'RNG and Gabriel filter pairs by geometric proximity. “Near” uses a maximum radius between centers.',
  near: 'Maximum radius between the centers of two rooms in “Near” mode. It is ignored in the other modes.',
  delta: 'Fraction of the cycle reference the generator tries to reach. 0 tends to a tree; larger values favor shortcuts and alternative routes.',
  stairsDoors: 'Maximum number of corridors that can enter the stairs room.',
  minHops: 'Minimum number of links between the entrance and the stairs, measured in rooms crossed.',
  ambush: 'Number of larger plain rooms that get the ambush role.',
  'topology.profile': 'Connectivity pattern: Classic keeps the current behavior; Backbone requires branches; Hub favors a central junction; Redundant exit requires paths with no shared stretches; Long loop requires a larger circuit.',
  'topology.minBranches': 'Minimum number of side components linked to the main path by a single passage.',
  'topology.minBranchDepth': 'Largest number of rooms a branch must have from the main path.',
  'topology.minExitRoutes': 'Minimum number of paths between the entrance and the stairs that share no corridor stretch.',
  'topology.minCycleLength': 'Minimum number of stretches in the shortest cycle of the graph. Use zero to impose no such requirement.',
  'topology.minHubDegree': 'Minimum number of arms at one or more points of the map. Use zero to not require a hub.',
  'progression.keyLocks': 'Number of gates where every path forward requires a key collected before the gate. The model accepts up to eight keys.',
  'progression.switchLocks': 'Number of gates where every path forward requires activating a switch before the gate. The model accepts up to eight switches.',
  'pacing.enabled': 'Spreads rest, boss and ambush rooms along the main path and favors prizes in optional branches.',
  leaves: 'Number of extra chambers the generator tries to place in free cells.',
  'leafRoles.trap': 'Relative weight for a leaf chamber to get a trap.',
  'leafRoles.prize': 'Relative weight for a leaf chamber to get a prize.',
  'leafRoles.empty': 'Relative weight for a leaf chamber to stay empty.',
  junction: 'Conductance between the arms of a junction. “∞” makes the arms behave as a single space. This parameter is informational and does not change the generation yet.',
}
for (const c of CLASSES) PARAM_HELP[`leafClasses.${c}`] = `Relative weight for choosing ${CLASS_LABELS[c]} rooms as leaf chambers.`
for (const a of CLASSES) for (const b of CLASSES) {
  const path = `mix.${mixKey(a, b)}`
  PARAM_HELP[path] = `Compatibility between ${CLASS_LABELS[a]} and ${CLASS_LABELS[b]} rooms. Zero forbids the link; one keeps the normal chance.`
}

const FIELD_OPTIONS = {
  m: [4, 8],
  discPorts: [4, 8, 16],
  maxBends: [0, 1, 2, 4, 5, 6],
  proximity: ['rng', 'gabriel', 'near'],
  'topology.profile': ['classic', 'backbone', 'hub', 'redundantExit', 'longLoop'],
}
const INTEGER_FIELDS = new Set(['cols', 'rows', 'big', 'starCells', 'discPorts', 'maxBends', 'stairsDoors', 'minHops', 'ambush', 'leaves', 'topology.minBranches', 'topology.minBranchDepth', 'topology.minExitRoutes', 'topology.minCycleLength', 'topology.minHubDegree', 'progression.keyLocks', 'progression.switchLocks', ...CLASSES.map((c) => `maxOf.${c}`)])
const BOOLEAN_FIELDS = new Set(['rotateRooms', 'pacing.enabled'])
const OPTION_LABELS = { classic: 'Classic', backbone: 'Backbone', hub: 'Hub', redundantExit: 'Redundant exit', longLoop: 'Long loop' }

const limitsFor = (path) => {
  if (path === 'cols' || path === 'rows') return { min: 1, max: 64 }
  if (path === 'cellW' || path === 'cellH') return { min: 1, max: 5000 }
  if (path === 'gap' || path === 'clear') return { min: 0, max: 1000 }
  if (path === 'drop' || path === 'delta') return { min: 0, max: 1 }
  if (path === 'layoutJitter') return { min: 0, max: 0.45 }
  if (path === 'homophily') return { min: -1, max: 1 }
  if (path === 'portStep' || path === 'width' || path === 'exit' || path === 'seg' || path === 'minLen' || path === 'near') return { min: 0.1, max: 10000 }
  if (path.startsWith('weights.') || path.startsWith('leafClasses.') || path.startsWith('leafRoles.')) return { min: 0, max: 1000 }
  if (path.startsWith('mix.')) return { min: 0, max: 1 }
  if (path.startsWith('p')) return { min: 0, max: 1000 }
  if (path.startsWith('cost')) return { min: 0, max: 1000 }
  if (path === 'big' || path === 'ambush' || path === 'leaves') return { min: 0, max: 1000 }
  if (path.startsWith('maxOf.')) return { min: 0, max: 10000 }
  if (path.startsWith('topology.')) return { min: 0, max: 1000 }
  if (path === 'progression.keyLocks' || path === 'progression.switchLocks') return { min: 0, max: 8 }
  if (path === 'starCells' || path === 'stairsDoors' || path === 'minHops') return { min: 1, max: 1000 }
  return { min: 0, max: 10000 }
}

const getPath = (object, path) => path.split('.').reduce((value, key) => value[key], object)
const setPath = (object, path, value) => {
  const keys = path.split('.')
  const last = keys.pop()
  keys.reduce((target, key) => target[key], object)[last] = value
}

const form = h('div', { class: 'parameter-groups' })
const changedCount = () => GROUPS.flatMap((g) => g.fields.map(([path]) => path)).filter((path) => getPath(state.P, path) !== getPath(state.defaults, path)).length
const updateChangedIndicator = () => {
  changedStatus.textContent = changedCount() === 0 ? 'Default configuration' : `${changedCount()} parameter(s) changed`
  changedStatus.classList.toggle('changed', changedCount() > 0)
}

const resetGroup = (group) => {
  for (const [path] of group.fields) setPath(state.P, path, clone(getPath(state.defaults, path)))
  buildForm()
  updateChangedIndicator()
  scheduleRegenerate()
}

const buildForm = () => {
  const expanded = new Set([...form.querySelectorAll('details[open]')].map((node) => node.dataset.group))
  form.replaceChildren()
  for (const group of GROUPS) {
    const box = h('details', { class: 'parameter-group', 'data-group': group.id })
    box.open = expanded.size ? expanded.has(group.id) : !!group.open
    const summary = h('summary', {}, h('span', {}, group.title), h('span', { class: 'group-count' }, `${group.fields.length}`))
    box.append(summary, h('p', { class: 'group-intro' }, group.intro))
    if (group.fields.length) {
      box.append(h('button', { type: 'button', class: 'text-button group-reset' }, 'Reset this group'))
      box.querySelector('.group-reset').addEventListener('click', () => resetGroup(group))
    }
    for (const [path, label] of group.fields) {
      const id = `param-${path.replaceAll('.', '-')}`
      const line = h('div', { class: 'field' })
      const options = FIELD_OPTIONS[path]
      const limits = limitsFor(path)
      const optionName = (option) => path === 'proximity'
        ? ({ rng: 'RNG', gabriel: 'Gabriel', near: 'Near' }[option])
        : (OPTION_LABELS[option] ?? String(option))
      const allowed = BOOLEAN_FIELDS.has(path)
        ? 'Check to enable; uncheck to disable.'
        : options
          ? `Options: ${options.map(optionName).join(', ')}.`
          : `Accepted range: ${formatNumber(limits.min, 2)} to ${formatNumber(limits.max, 2)}${INTEGER_FIELDS.has(path) ? ', whole numbers' : ''}.`
      const helpText = `${PARAM_HELP[path] ?? `Parameter ${label}.`} ${allowed}`
      const title = h('div', { class: 'field-title' }, h('label', { for: id }, label), help(helpText))
      const error = h('span', { class: 'field-error', 'aria-live': 'polite' })
      let input
      if (BOOLEAN_FIELDS.has(path)) {
        input = h('input', { id, name: path, type: 'checkbox', 'aria-label': label })
        input.checked = !!getPath(state.P, path)
        input.addEventListener('change', () => {
          setPath(state.P, path, input.checked)
          updateChangedIndicator()
          scheduleRegenerate()
        })
      } else if (options) {
        input = h('select', { id, name: path })
        for (const option of options) input.append(h('option', { value: String(option) }, optionName(option)))
        input.value = String(getPath(state.P, path))
        input.addEventListener('change', () => {
          setPath(state.P, path, typeof getPath(state.P, path) === 'number' ? Number(input.value) : input.value)
          updateChangedIndicator()
          scheduleRegenerate()
        })
      } else {
        const limits = limitsFor(path)
        input = h('input', { id, name: path, type: 'number', value: String(getPath(state.P, path)), min: String(limits.min), max: String(limits.max), step: INTEGER_FIELDS.has(path) ? '1' : 'any', inputmode: 'decimal' })
        input.addEventListener('change', () => {
          const value = Number(input.value)
          const integerRequired = INTEGER_FIELDS.has(path)
          if (!input.value || !Number.isFinite(value) || value < limits.min || value > limits.max || (integerRequired && !Number.isInteger(value))) {
            error.textContent = integerRequired ? `Use an integer between ${limits.min} and ${limits.max}.` : `Use a value between ${limits.min} and ${limits.max}.`
            input.setAttribute('aria-invalid', 'true')
            return
          }
          error.textContent = ''
          input.removeAttribute('aria-invalid')
          setPath(state.P, path, value)
          updateChangedIndicator()
          scheduleRegenerate()
        })
      }
      line.append(title, input, error)
      box.append(line)
    }
    form.append(box)
  }
  const junction = h('details', { class: 'parameter-group', 'data-group': 'junction-info' }, h('summary', {}, h('span', {}, 'Junction conductance'), h('span', { class: 'group-count' }, 'info')))
  const junctionValue = state.P.junction == null ? '∞ (arms joined)' : formatNumber(state.P.junction, 2)
  junction.append(h('p', { class: 'group-intro' }, PARAM_HELP.junction), h('div', { class: 'readonly-setting' }, h('span', {}, 'Current value'), h('strong', {}, junctionValue), help(PARAM_HELP.junction)))
  form.append(junction)
}

/* ---------------- Map drawing ---------------- */

const canvas = h('canvas', { id: 'dungeon-map', tabindex: '0', role: 'img', 'aria-label': 'Floor map. Use the mouse to explore the rooms and corridors.' })
const ctx = canvas.getContext('2d')
const mapTooltip = h('div', { class: 'map-tooltip', role: 'tooltip', 'aria-live': 'polite', hidden: '' })
const zoomLabel = h('span', { class: 'zoom-label', 'aria-live': 'polite' }, '100%')
const viewSection = h('section', { id: 'view', 'aria-label': 'Map view' }, canvas, mapTooltip)

const pathOf = (shape) => {
  ctx.beginPath()
  if (shape.kind === 'disc') ctx.arc(shape.c[0], shape.c[1], shape.r, 0, Math.PI * 2)
  else shape.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
  ctx.closePath()
}

const transformOf = () => {
  const W = canvas.clientWidth
  const H = canvas.clientHeight
  const P = current?.dungeon.params
  if (!P) return { W, H, k: 1, ox: 0, oy: 0, worldW: 1, worldH: 1 }
  const worldW = P.cols * P.cellW
  const worldH = P.rows * P.cellH
  const fit = Math.min(Math.max(1, W - 48) / worldW, Math.max(1, H - 48) / worldH)
  const k = fit * view.zoom
  return { W, H, k, ox: W / 2 + view.x - (worldW / 2) * k, oy: H / 2 + view.y - (worldH / 2) * k, worldW, worldH }
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
  if (!current) return
  const { W: width, H: height, k, ox, oy, worldW, worldH } = transformOf()
  ctx.setTransform(dpr * k, 0, 0, dpr * k, dpr * ox, dpr * oy)
  const px = 1 / k
  const d = current.dungeon
  const P = d.params

  if (overlays.cells) {
    ctx.strokeStyle = 'rgba(153, 181, 215, .12)'
    ctx.lineWidth = px
    const gridX = d.gridX?.length === P.cols + 1 ? d.gridX : Array.from({ length: P.cols + 1 }, (_, i) => i * P.cellW)
    const gridY = d.gridY?.length === P.rows + 1 ? d.gridY : Array.from({ length: P.rows + 1 }, (_, i) => i * P.cellH)
    for (const x of gridX) (ctx.beginPath(), ctx.moveTo(x, gridY[0]), ctx.lineTo(x, gridY.at(-1)), ctx.stroke())
    for (const y of gridY) (ctx.beginPath(), ctx.moveTo(gridX[0], y), ctx.lineTo(gridX.at(-1), y), ctx.stroke())
  }
  if (overlays.pairs) {
    ctx.strokeStyle = 'rgba(120, 170, 235, .48)'
    ctx.setLineDash([7 * px, 6 * px])
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
    ctx.strokeStyle = 'rgba(94, 191, 255, .42)'
    ctx.lineWidth = 2 * px
    for (const candidate of d.candidates) {
      ctx.beginPath()
      candidate.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
      ctx.stroke()
    }
  }
  const hasCorridorRegions = d.edges.every((edge) => Array.isArray(edge.region))
  if (hasCorridorRegions) {
    ctx.fillStyle = '#9a8060'
    for (const edge of d.edges) for (const shape of edge.region) (pathOf(shape), ctx.fill())
  } else {
    ctx.strokeStyle = '#9a8060'
    ctx.lineWidth = P.width
    ctx.lineCap = 'butt'
    ctx.lineJoin = 'round'
    for (const edge of d.edges) {
      ctx.beginPath()
      edge.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
      ctx.stroke()
    }
  }
  ctx.strokeStyle = 'rgba(34, 25, 18, .58)'
  ctx.lineWidth = px
  ctx.lineCap = 'butt'
  ctx.lineJoin = 'round'
  for (const edge of d.edges) {
    ctx.beginPath()
    edge.pts.forEach(([x, y], i) => (i ? ctx.lineTo(x, y) : ctx.moveTo(x, y)))
    ctx.stroke()
  }
  for (const edge of d.edges) {
    if (!edge.requiresKeys && !edge.requiresSwitches) continue
    const lengths = edge.pts.slice(1).map((point, i) => Math.hypot(point[0] - edge.pts[i][0], point[1] - edge.pts[i][1]))
    let remaining = lengths.reduce((sum, length) => sum + length, 0) / 2
    let at = edge.pts[0]
    for (let i = 0; i < lengths.length; i++) {
      if (remaining <= lengths[i]) {
        const t = lengths[i] ? remaining / lengths[i] : 0
        at = [edge.pts[i][0] + (edge.pts[i + 1][0] - edge.pts[i][0]) * t, edge.pts[i][1] + (edge.pts[i + 1][1] - edge.pts[i][1]) * t]
        break
      }
      remaining -= lengths[i]
    }
    ctx.beginPath()
    ctx.arc(at[0], at[1], 8 * px, 0, Math.PI * 2)
    ctx.fillStyle = '#18212b'
    ctx.fill()
    ctx.strokeStyle = '#ffd36d'
    ctx.lineWidth = 2 * px
    ctx.stroke()
    ctx.font = `bold ${9 * px}px system-ui, sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'middle'
    ctx.fillStyle = '#ffe9ae'
    ctx.fillText(edge.requiresKeys ? 'K' : 'S', at[0], at[1] + 0.5 * px)
  }
  for (const room of d.rooms) {
    if (!room.alive) continue
    pathOf(room.shape)
    ctx.fillStyle = COLORS[room.cls] ?? '#59677a'
    ctx.globalAlpha = room.leaf ? 0.62 : 0.9
    ctx.fill()
    ctx.globalAlpha = 1
    ctx.strokeStyle = ROLE_COLORS[room.role] ?? '#18212d'
    ctx.lineWidth = (ROLE_COLORS[room.role] ? 3.5 : 1.5) * px
    ctx.stroke()
  }
  for (const room of d.rooms) {
    if (room.alive) continue
    pathOf(room.shape)
    ctx.strokeStyle = '#fb7474'
    ctx.setLineDash([4 * px, 4 * px])
    ctx.lineWidth = px
    ctx.stroke()
    ctx.setLineDash([])
  }
  if (overlays.ports) {
    ctx.fillStyle = 'rgba(255, 255, 255, .28)'
    for (const room of d.rooms) for (const port of room.ports) ctx.fillRect(port.p[0] - 1.5 * px, port.p[1] - 1.5 * px, 3 * px, 3 * px)
  }
  ctx.strokeStyle = '#fff'
  ctx.lineWidth = 2 * px
  for (const room of d.rooms) for (const port of room.used) {
    const direction = DIRS[port.k]
    ctx.beginPath()
    ctx.moveTo(port.p[0], port.p[1])
    ctx.lineTo(port.p[0] + direction[0] * 14, port.p[1] + direction[1] * 14)
    ctx.stroke()
  }
  for (const node of d.nodes) {
    if (node.kind !== 'junction') continue
    ctx.beginPath()
    ctx.arc(node.p[0], node.p[1], 5, 0, Math.PI * 2)
    ctx.fillStyle = '#fff'
    ctx.fill()
    ctx.strokeStyle = '#202a38'
    ctx.lineWidth = px
    ctx.stroke()
  }
  if (overlays.labels) {
    ctx.font = `${12 * px}px system-ui, sans-serif`
    ctx.textAlign = 'center'
    ctx.textBaseline = 'alphabetic'
    for (const room of d.rooms) {
      if (!room.alive) continue
      const inventory = [room.keys ? `K${room.keys.toString(2).replaceAll('0', '').length}` : '', room.switches ? `S${room.switches.toString(2).replaceAll('0', '').length}` : ''].filter(Boolean).join(' ')
      const label = `#${room.id} ${CLASS_LABELS[room.cls] ?? room.cls}${room.role !== 'plain' ? ` · ${ROLE_LABELS[room.role]}` : ''}${inventory ? ` · ${inventory}` : ''}`
      ctx.lineWidth = 3 * px
      ctx.strokeStyle = '#080d14'
      ctx.strokeText(label, room.center[0], room.center[1] + 4 * px)
      ctx.fillStyle = '#f1f5fb'
      ctx.fillText(label, room.center[0], room.center[1] + 4 * px)
    }
  }
  zoomLabel.textContent = `${Math.round(view.zoom * 100)}%`
}

/* ---------------- Details while exploring the map ---------------- */

const containsPoint = (shape, point) => {
  if (shape.kind === 'disc') return Math.hypot(point[0] - shape.c[0], point[1] - shape.c[1]) <= shape.r
  let inside = false
  const pts = shape.pts
  for (let i = 0, j = pts.length - 1; i < pts.length; j = i++) {
    const [xi, yi] = pts[i]
    const [xj, yj] = pts[j]
    const crosses = (yi > point[1]) !== (yj > point[1]) && point[0] < ((xj - xi) * (point[1] - yi)) / (yj - yi) + xi
    if (crosses) inside = !inside
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

const roomRoleText = (room) => room.leaf ? `leaf chamber · ${ROLE_LABELS[room.role] ?? 'empty'}` : (ROLE_LABELS[room.role] ?? 'plain room')
const nodeLabel = (dungeon, nodeId) => {
  const node = dungeon.nodes[nodeId]
  if (node?.kind === 'junction') return 'junction'
  const room = dungeon.rooms[node?.room]
  return room ? `room #${room.id}` : 'point'
}

const mapObjectAt = (point, scale) => {
  const d = current.dungeon
  for (const room of [...d.rooms].reverse()) {
    if (room.alive && containsPoint(room.shape, point)) {
      const inventory = [room.keys ? `keys: ${room.keys.toString(2).replaceAll('0', '').length}` : '', room.switches ? `switches: ${room.switches.toString(2).replaceAll('0', '').length}` : ''].filter(Boolean)
      return `${roomRoleText(room)}\nRoom #${room.id} · ${CLASS_LABELS[room.cls] ?? room.cls}\nArea: ${formatNumber(room.area, 0)} px²\nDoors used: ${room.used.length} · possible doors: ${room.ports.length}${inventory.length ? `\nItems: ${inventory.join(' · ')}` : ''}`
    }
  }
  const junction = d.nodes.find((node) => node.kind === 'junction' && Math.hypot(node.p[0] - point[0], node.p[1] - point[1]) <= Math.max(6, 8 / scale))
  if (junction) {
    const id = d.nodes.indexOf(junction)
    const degree = d.edges.filter((edge) => edge.u === id || edge.v === id).length
    return `Corridor junction\n${degree} arms connected\nPosition: ${formatNumber(junction.p[0], 0)}, ${formatNumber(junction.p[1], 0)} px`
  }
  const threshold = current.dungeon.params.width / 2 + 5 / scale
  for (const edge of d.edges) {
    for (let i = 1; i < edge.pts.length; i++) {
      if (distanceToSegment(point, edge.pts[i - 1], edge.pts[i]) <= threshold) {
        const length = edge.pts.slice(1).reduce((sum, p, j) => sum + Math.hypot(p[0] - edge.pts[j][0], p[1] - edge.pts[j][1]), 0)
        const gates = [
          edge.requiresKeys ? `key${edge.requiresKeys.toString(2).replaceAll('0', '').length > 1 ? 's' : ''} ${edge.requiresKeys.toString(2)}` : '',
          edge.requiresSwitches ? `switch${edge.requiresSwitches.toString(2).replaceAll('0', '').length > 1 ? 'es' : ''} ${edge.requiresSwitches.toString(2)}` : '',
        ].filter(Boolean)
        return `Corridor segment #${edge.id}\n${nodeLabel(d, edge.u)} → ${nodeLabel(d, edge.v)}\n${edge.pts.length - 1} segment(s) · ${formatNumber(length, 0)} px${gates.length ? `\nLocked: requires ${gates.join(' and ')}` : ''}`
      }
    }
  }
  return ''
}

const showMapTooltip = (event) => {
  if (!current) return
  const rect = canvas.getBoundingClientRect()
  const { k, ox, oy } = transformOf()
  const point = [(event.clientX - rect.left - ox) / k, (event.clientY - rect.top - oy) / k]
  const content = mapObjectAt(point, k)
  if (!content) {
    mapTooltip.hidden = true
    return
  }
  mapTooltip.textContent = content
  mapTooltip.hidden = false
  const viewRect = viewSection.getBoundingClientRect()
  mapTooltip.style.left = `${Math.max(10, Math.min(event.clientX - viewRect.left + 14, viewRect.width - 250))}px`
  mapTooltip.style.top = `${Math.max(58, Math.min(event.clientY - viewRect.top + 14, viewRect.height - 100))}px`
}

const hideMapTooltip = () => { mapTooltip.hidden = true }

/* ---------------- Metrics panel ---------------- */

const right = h('section', { id: 'results', class: 'panel', 'aria-label': 'Floor validation and measures' })
const METRIC_INFO = {
  rooms: 'Main rooms of the floor. Leaf chambers are listed separately.',
  leafChambers: 'Extra rooms hanging off a single corridor, like a dead end.',
  junctions: 'Points where three or four corridor arms meet.',
  edges: 'Graph stretches between rooms and junctions. Each arm of a junction counts as a stretch.',
  beta: 'Number of independent cycles. A tree has zero; each new shortcut usually adds one.',
  betaTarget: 'Cycle target computed from the chosen density and the generator reference.',
  betaMax: 'Greedy reference for how many cycles fit. Considers direct links and allowed junctions.',
  delta: 'Ratio between current cycles and the βmax reference. A larger value usually means more alternative routes.',
  meanDegree: 'Average number of corridor doors used per room.',
  leaves: 'Rooms with only one door. Usually dead ends, the entrance or the stairs.',
  bridges: 'Stretches whose removal would split off part of the map. Many of them indicate bottlenecks.',
  articulations: 'Rooms whose removal would split the map into disconnected parts.',
  hopsToStairs: 'Number of rooms crossed on the shortest path from the entrance to the stairs.',
  diameter: 'Largest minimum distance, in steps, between two rooms of the map.',
  routesToStairs: 'Number of paths between the entrance and the stairs that share no corridor stretch.',
  cycleGirth: 'Length, in stretches, of the shortest cycle of the map. “—” means the graph has no cycles.',
  sideBranches: 'Number of side components attached to the main path by a single passage.',
  deepestBranch: 'Largest number of rooms in a side branch, counting the room joined to the main path.',
  maxNodeDegree: 'Largest number of corridor arms joined at one point or room.',
  geometricPathToStairs: 'Total length in pixels of the path with the fewest stretches between the entrance and the stairs.',
  lambda2: 'Connectivity measure: larger values tend to indicate a map less dependent on bottlenecks.',
}

const metricRow = (label, value, description) => h('tr', {}, h('th', { scope: 'row' }, label, help(description)), h('td', {}, value))

const translateViolation = (message) => {
  let match
  if ((match = message.match(/^V1 room (\d+) not reached from the start$/))) return `Room #${match[1]} cannot be reached from the entrance.`
  if ((match = message.match(/^V2 rooms (\d+) and (\d+) closer than 2g$/))) return `Rooms #${match[1]} and #${match[2]} are too close together.`
  if ((match = message.match(/^V4 corridor (.+) reaches (\d+) room\(s\): every corridor joins rooms$/))) return `Corridor ${match[1]} reaches only ${match[2]} room(s); every corridor must join rooms.`
  if ((match = message.match(/^V4 rooms (\d+) and (\d+): corridor (.+) px < ℓmin$/))) return `The path between rooms #${match[1]} and #${match[2]} is too short (${match[3]} px).`
  if ((match = message.match(/^V2 rooms (\d+) and (\d+) joined but closer than ℓmin in a straight line$/))) return `Rooms #${match[1]} and #${match[2]} are too close in a straight line.`
  if ((match = message.match(/^V2 corridor (\d+) runs into room (\d+) past its door$/))) return `Corridor #${match[1]} runs into room #${match[2]} past the door.`
  if ((match = message.match(/^V2 corridor (\d+) closer than σ to room (\d+)$/))) return `Corridor #${match[1]} is too close to room #${match[2]}.`
  if ((match = message.match(/^V2 corridors (\d+) and (\d+) closer than σ$/))) return `Corridors #${match[1]} and #${match[2]} are too close together.`
  if ((match = message.match(/^V3 room (\d+): a door at 45° in a retilinear stage$/))) return `Room #${match[1]} has a diagonal door on a rectilinear floor.`
  if ((match = message.match(/^V3 room (\d+): a door's exit band runs back into the room$/))) return `The exit of a door of room #${match[1]} runs back into the room.`
  if ((match = message.match(/^V3 room (\d+): a door off the circle$/))) return `A door of circular room #${match[1]} is not on the edge.`
  if ((match = message.match(/^V3 room (\d+): doors (\d+) and (\d+) closer than σ$/))) return `Two doors of room #${match[1]} are too close together.`
  if ((match = message.match(/^V4 corridor (\d+): a segment off the alphabet$/))) return `Corridor #${match[1]} uses a direction that is not allowed.`
  if ((match = message.match(/^V4 corridor (\d+): a (\d+)° turn$/))) return `Corridor #${match[1]} has a ${match[2]}° bend, which is not allowed.`
  if ((match = message.match(/^V4 corridor (\d+): a segment of (.+) px$/))) return `A segment of corridor #${match[1]} is too short (${match[2]} px).`
  if ((match = message.match(/^V4 corridor (\d+): leaves a door at an angle$/))) return `Corridor #${match[1]} leaves a door in the wrong direction.`
  if ((match = message.match(/^V5 the stairs have (\d+) doors \(most (\d+)\)$/))) return `The stairs have ${match[1]} doors; the configured maximum is ${match[2]}.`
  if ((match = message.match(/^V5 the stairs are (.+) hops from the start \(least (\d+)\)$/))) return `The stairs are ${match[1]} steps from the entrance; the minimum is ${match[2]}.`
  if (message === 'V5 start and stairs must be two rooms of the stage') return 'The entrance and the stairs must be two distinct rooms of the floor.'
  if ((match = message.match(/^T1 profile requires (\d+) side branches; found (\d+)$/))) return `The profile requires ${match[1]} side branches; ${match[2]} were generated.`
  if ((match = message.match(/^T2 profile requires branch depth (\d+); found (\d+)$/))) return `The profile requires a branch depth of ${match[1]}; ${match[2]} was generated.`
  if ((match = message.match(/^T3 profile requires (\d+) independent routes to stairs; found (\d+)$/))) return `The profile requires ${match[1]} independent routes to the stairs; ${match[2]} were generated.`
  if ((match = message.match(/^T4 profile requires a hub degree of (\d+); found (\d+)$/))) return `The profile requires a point with at least ${match[1]} arms; the largest has ${match[2]}.`
  if ((match = message.match(/^T5 profile requires a cycle of at least (\d+) branches; found (\d+)$/))) return `The profile requires a cycle of at least ${match[1]} stretches; the shortest has ${match[2]}.`
  if (message === 'P1 stairs are unreachable with the generated keys and switches') return 'The stairs cannot be reached with the available keys and switches.'
  if (message === 'P2 a required key is unreachable before its gate') return 'A required key cannot be collected before its gate.'
  if (message === 'P3 a required switch is unreachable before its gate') return 'A required switch cannot be activated before its gate.'
  return message
}

const translateLog = (message) => {
  let match
  if ((match = message.match(/^tree stuck: candidates from (.+)$/i))) return `Few connections available; widening the search (${match[1]}).`
  if ((match = message.match(/^tree stuck: near (.+) px$/i))) return `Widening the search radius to ${match[1]} px.`
  if ((match = message.match(/^(.+) room\(s\) left out: no valid corridor reaches them$/))) return `${match[1]} room(s) were left isolated because no valid link was found.`
  if ((match = message.match(/^stairs only (.+) hops from the start \(wanted (.+)\)$/))) return `The stairs ended up ${match[1]} steps from the entrance; the target was ${match[2]}.`
  if ((match = message.match(/^β\* = (.+) not reached: nothing more fits \(β = (.+)\)$/))) return `The target of ${match[1]} cycles was not reached; the map ended with ${match[2]}.`
  if ((match = message.match(/^block (\d+): (.+) does not fit, rect instead$/))) return `The ${match[2]} shape did not fit in block ${match[1]}; a rectangular room was used.`
  if ((match = message.match(/^(.+) of (.+) leaf chambers fit$/))) return `${match[1]} of ${match[2]} leaf chambers fit.`
  if ((match = message.match(/^progression: only (\d+) of (\d+) requested locks fit on the main path$/))) return `Only ${match[1]} of the ${match[2]} requested locks fit on the main path.`
  if (message === 'progression: each lock type is limited to eight bits') return 'Each lock type accepts at most eight items.'
  if ((match = message.match(/^topology profile: could not reach (\d+) independent routes to stairs \(λ = (\d+)\)$/))) return `The profile reached ${match[2]} of ${match[1]} independent routes to the stairs.`
  if ((match = message.match(/^topology profile: no valid junction can reach degree (\d+)$/))) return `No junction of degree ${match[1]} fit.`
  if ((match = message.match(/^topology profile: no valid cycle of at least (\d+) branches fits$/))) return `No cycle of at least ${match[1]} stretches fit.`
  if ((match = message.match(/^topology profile: added (\d+) of (\d+) required side branches$/))) return `Added ${match[1]} of the ${match[2]} required side branches.`
  if ((match = message.match(/^layout jitter limited to (.+) \(requested (.+)\) to preserve minimum room size$/))) return `Grid irregularity was limited to ${match[1]} (requested: ${match[2]}) to preserve the minimum room size.`
  if (/shape class caps cannot fit/i.test(message)) return 'The shape limits cannot hold all the rooms; some limit was exceeded.'
  return message
}

const renderPanel = () => {
  right.replaceChildren()
  if (!current) return
  const result = current
  const m = result.metrics
  const status = result.violations.length === 0
    ? h('div', { class: 'validity-card valid' }, h('span', { class: 'status-icon', 'aria-hidden': 'true' }, '✓'), h('div', {}, h('strong', {}, 'Valid map'), h('p', {}, `${result.draws} attempt(s) · ${formatNumber(result.ms, 1)} ms to generate`)))
    : h('div', { class: 'validity-card invalid' }, h('span', { class: 'status-icon', 'aria-hidden': 'true' }, '!'), h('div', {}, h('strong', {}, `${result.violations.length} problem(s) found`), h('p', {}, `After ${result.draws} attempt(s), the map still does not meet every rule.`), h('ul', {}, ...result.violations.slice(0, 30).map((item) => h('li', {}, translateViolation(item))))))
  right.append(h('div', { class: 'panel-header' }, h('p', { class: 'eyebrow' }, `LEVEL ${state.level} · SEED ${state.seed}`), h('h2', {}, 'Floor reading')), status)

  const generalRows = [
    ['Main rooms', `${m.rooms}`, METRIC_INFO.rooms],
    ['Leaf chambers', `${m.leafChambers}`, METRIC_INFO.leafChambers],
    ['T/X junctions', `${m.junctions}`, METRIC_INFO.junctions],
    ['Corridor segments', `${m.edges}`, METRIC_INFO.edges],
  ]
  const cycleRows = [
    ['Current cycles β', `${m.beta}`, METRIC_INFO.beta],
    ['Cycle target', `${m.betaTarget}`, METRIC_INFO.betaTarget],
    ['Reference βmax', `${m.betaMax}`, METRIC_INFO.betaMax],
    ['Density δ', `${formatNumber(m.delta, 2)}`, METRIC_INFO.delta],
  ]
  const routeRows = [
    ['Doors used per room', formatNumber(m.meanDegree, 2), METRIC_INFO.meanDegree],
    ['Single-door rooms', `${m.leaves}`, METRIC_INFO.leaves],
    ['Bridges (share)', `${m.bridges} · ${formatNumber(m.bridgeShare * 100, 0)}%`, METRIC_INFO.bridges],
    ['Articulation rooms', `${m.articulations}`, METRIC_INFO.articulations],
    ['Steps to the stairs', m.hopsToStairs == null ? '—' : formatNumber(m.hopsToStairs, 0), METRIC_INFO.hopsToStairs],
    ['Map diameter', formatNumber(m.diameter, 0), METRIC_INFO.diameter],
    ['Independent routes to the stairs', `${m.routesToStairs}`, METRIC_INFO.routesToStairs],
    ['Shortest cycle (stretches)', m.cycleGirth == null ? '—' : `${m.cycleGirth}`, METRIC_INFO.cycleGirth],
    ['Side branches', `${m.sideBranches}`, METRIC_INFO.sideBranches],
    ['Branch depth', `${m.deepestBranch}`, METRIC_INFO.deepestBranch],
    ['Highest degree of a point', `${m.maxNodeDegree}`, METRIC_INFO.maxNodeDegree],
    ['Geometric path to the stairs', `${formatNumber(m.geometricPathToStairs, 0)} px`, METRIC_INFO.geometricPathToStairs],
    ['Connectivity λ₂', formatNumber(m.lambda2, 3), METRIC_INFO.lambda2],
  ]
  const tableFor = (rows) => h('table', { class: 'metric-table' }, h('tbody', {}, ...rows.map(([label, value, description]) => metricRow(label, value, description))))
  right.append(
    h('details', { class: 'result-group', open: '' }, h('summary', {}, 'Map size'), tableFor(generalRows)),
    h('details', { class: 'result-group', open: '' }, h('summary', {}, 'Cycles and density'), tableFor(cycleRows)),
    h('details', { class: 'result-group' }, h('summary', {}, 'Paths and connectivity'), tableFor(routeRows)),
  )

  const classCounts = Object.entries(m.byClass).map(([name, count]) => h('span', { class: 'shape-chip' }, h('i', { style: `background:${COLORS[name] ?? '#59677a'}`, 'aria-hidden': 'true' }), `${CLASS_LABELS[name] ?? name}: ${count}`))
  right.append(h('section', { class: 'result-section' }, h('h3', {}, 'Shapes on the map'), h('div', { class: 'chip-list' }, ...classCounts)))

  if (result.dungeon.log.length) {
    right.append(h('details', { class: 'result-group' }, h('summary', {}, 'What the generator did'), h('ul', { class: 'log-list' }, ...result.dungeon.log.map((line) => h('li', {}, translateLog(line))))))
  }

  const shapeLegend = CLASSES.map((name) => h('span', { class: 'legend-item' }, h('i', { style: `background:${COLORS[name]}`, 'aria-hidden': 'true' }), CLASS_LABELS[name]))
  const roleLegend = Object.keys(ROLE_COLORS).map((name) => h('span', { class: 'legend-item' }, h('i', { style: `background:${ROLE_COLORS[name]}`, 'aria-hidden': 'true' }), ROLE_LABELS[name]))
  right.append(h('details', { class: 'result-group' }, h('summary', {}, 'Map legend'), h('div', { class: 'legend-block' }, h('strong', {}, 'Shapes'), h('div', { class: 'legend-list' }, ...shapeLegend), h('strong', {}, 'Room roles'), h('div', { class: 'legend-list' }, ...roleLegend), h('p', { class: 'muted' }, 'White strokes show used doors; white dots are junctions. Rooms dashed in red were left out of the network.'))))
}

/* ---------------- Controls and header ---------------- */

const levelButtons = [1, 2, 3, 4, 5].map((level) => {
  const button = h('button', { type: 'button', class: 'level-button', 'aria-label': `Use the level ${level} configuration` }, `Level ${level}`)
  button.addEventListener('click', () => setLevel(level))
  return button
})
const seedInput = h('input', { type: 'number', min: '0', max: '4294967295', step: '1', value: '1', 'aria-label': 'Random seed' })
const levelDescription = h('p', { class: 'level-description' }, LEVEL_INFO[0])
const changedStatus = h('span', { class: 'changed-status' }, 'Default configuration')
const toast = h('div', { class: 'toast', role: 'status', 'aria-live': 'polite', hidden: '' })

const showToast = (message) => {
  toast.textContent = message
  toast.hidden = false
  window.clearTimeout(showToast.timer)
  showToast.timer = window.setTimeout(() => { toast.hidden = true }, 2200)
}

const updateLevelButtons = () => {
  levelButtons.forEach((button, index) => {
    const selected = index + 1 === state.level
    button.classList.toggle('selected', selected)
    button.setAttribute('aria-pressed', String(selected))
  })
  levelDescription.textContent = LEVEL_INFO[state.level - 1]
}

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

seedInput.addEventListener('change', () => {
  const seed = Number(seedInput.value)
  if (!Number.isInteger(seed) || seed < 0 || seed > 0xffffffff) {
    seedInput.setAttribute('aria-invalid', 'true')
    seedInput.value = String(state.seed)
    showToast('The seed must be an integer between 0 and 4,294,967,295.')
    return
  }
  seedInput.removeAttribute('aria-invalid')
  state.seed = seed
  scheduleRegenerate()
})

const performanceStatus = h('div', { class: 'status-line', 'aria-live': 'polite' }, 'Benchmarks use the default parameters of the selected level.')
const performanceGroup = h('details', { class: 'tool-group' }, h('summary', {}, 'Performance and export'), h('p', { class: 'group-intro' }, 'Benchmarks use the level default configuration, not the values changed in the form.'), h('div', { class: 'tool-buttons' }))
const toolButtons = performanceGroup.querySelector('.tool-buttons')

toolButtons.append(
  h('button', { type: 'button', class: 'secondary-button' }, 'Measure 40 maps'),
  h('button', { type: 'button', class: 'secondary-button' }, 'Measure batch of 800'),
)
toolButtons.children[0].addEventListener('click', () => {
  const result = JSON.parse(bench(state.level, 1, 40))
  performanceStatus.textContent = `Average: ${formatNumber(result.ms / 40, 2)} ms per map · ${result.invalid} invalid of 40.`
})
toolButtons.children[1].addEventListener('click', async () => {
  performanceStatus.textContent = 'Measuring the batch…'
  try {
    const one = await pool.batch(state.level, 1, 800, 1)
    const all = await pool.batch(state.level, 1, 800)
    performanceStatus.textContent = `800 maps: 1 worker ${formatNumber(one.ms, 0)} ms · ${all.workers} workers ${formatNumber(all.ms, 0)} ms · ${all.invalid} invalid.`
  } catch (error) {
    performanceStatus.textContent = `Could not measure the batch: ${error}`
  }
})
performanceGroup.append(performanceStatus)

const overlaysHelp = {
  cells: 'Shows the borders of the cells that make up the floor grid.',
  labels: 'Shows the number, shape and role of each room.',
  ports: 'Shows every possible door position, including the ones that were not used.',
  pairs: 'Shows the pairs of rooms that entered the connection search.',
  candidates: 'Shows the candidate paths available to the generator.',
}
const overlayNames = { cells: 'Grid', labels: 'Room names', ports: 'Possible doors', pairs: 'Pairs considered', candidates: 'Candidate paths' }
const overlayList = h('div', { class: 'overlay-list' })
for (const key of Object.keys(overlays)) {
  const input = h('input', { type: 'checkbox', checked: overlays[key] ? 'checked' : '', 'aria-label': overlayNames[key] })
  input.checked = overlays[key]
  input.addEventListener('change', () => { overlays[key] = input.checked; draw() })
  overlayList.append(h('label', { class: 'overlay-option' }, input, h('span', {}, overlayNames[key]), help(overlaysHelp[key])))
}

const experiments = h('details', { class: 'tool-group experiments' }, h('summary', {}, 'Guided experiments'), h('p', { class: 'group-intro' }, 'Apply a sample change to see its effect. It only replaces the parameters involved; you can edit the rest as usual.'), h('div', { class: 'experiment-buttons' }))
const experimentButtons = experiments.querySelector('.experiment-buttons')
const runExperiment = (kind) => {
  if (kind === 'shapes') {
    for (const shape of CLASSES) state.P.weights[shape] = 1
    state.P.m = 8
    showToast('Experiment applied: varied shapes and diagonal directions.')
  } else if (kind === 'cycles') {
    state.P.delta = 0.8
    showToast('Experiment applied: high cycle density.')
  } else if (kind === 'junctions') {
    state.P.p_t = Math.max(state.P.p_t, 0.25)
    state.P.p_x = Math.max(state.P.p_x, 0.15)
    showToast('Experiment applied: T and X junctions favored.')
  } else if (kind === 'rooms') {
    for (const shape of CLASSES) state.P.weights[shape] = shape === 'disc' ? 1 : 0
    showToast('Experiment applied: preference for circular rooms.')
  }
  buildForm()
  updateChangedIndicator()
  scheduleRegenerate()
}
for (const [kind, label, detail] of [
  ['shapes', 'Mix shapes', 'Sets equal weights for the shapes and enables the octilinear alphabet.'],
  ['cycles', 'Create more routes', 'Raises the target density to favor cycles and shortcuts.'],
  ['junctions', 'Show T/X junctions', 'Raises the weights of three- and four-arm junctions.'],
  ['rooms', 'Use circular rooms', 'Makes circular rooms dominate the draw.'],
]) {
  const button = h('button', { type: 'button', class: 'experiment-button' }, h('strong', {}, label), h('span', {}, detail))
  button.addEventListener('click', () => runExperiment(kind))
  experimentButtons.append(button)
}

const copyButton = h('button', { type: 'button', class: 'secondary-button' }, 'Copy parameters (JSON)')
copyButton.addEventListener('click', async () => {
  try {
    await navigator.clipboard.writeText(JSON.stringify({ level: state.level, seed: state.seed, params: state.P }, null, 2))
    showToast('Parameters copied as JSON.')
  } catch {
    showToast('The clipboard is not available in this browser.')
  }
})

const restoreButton = h('button', { type: 'button', class: 'secondary-button' }, 'Reset to level default')
restoreButton.addEventListener('click', () => setLevel(state.level))

const left = h('section', { id: 'controls', class: 'panel', 'aria-label': 'Generator controls' })
left.append(
  h('div', { class: 'brand-row' }, h('div', { class: 'brand-mark', 'aria-hidden': 'true' }, '⌘'), h('div', {}, h('p', { class: 'eyebrow' }, 'INTERACTIVE LAB'), h('h1', {}, 'Tapiti'))),
  h('p', { class: 'intro-copy' }, 'Explore how the mathematical rules change the shape and paths of a floor. Change a value to generate a new map.'),
  h('div', { class: 'level-section' }, h('div', { class: 'section-title' }, h('h2', {}, 'Choose a level'), help('Each level is a starting set of parameters. You can edit any value afterwards.')), h('div', { class: 'level-list' }, ...levelButtons), levelDescription),
  h('div', { class: 'seed-card' }, h('label', { for: 'seed-input' }, 'Map seed', help('The same seed and parameters generate the same map. Change the seed to explore another random configuration.')), h('div', { class: 'seed-control' }, seedInput, h('button', { type: 'button', class: 'icon-button', 'aria-label': 'Previous seed', title: 'Previous seed' }, '←'), h('button', { type: 'button', class: 'icon-button', 'aria-label': 'Next seed', title: 'Next seed' }, '→'), h('button', { type: 'button', class: 'secondary-button' }, 'New seed')), h('p', { class: 'keyboard-hint' }, 'Shortcuts: 1–5 levels · ← → seeds · R new seed')),
  h('div', { class: 'action-row' }, copyButton, restoreButton),
  h('div', { class: 'changed-row' }, changedStatus),
  h('details', { class: 'howto' }, h('summary', {}, 'How to explore this map'), h('p', {}, 'Drag the map to move it. Use the mouse wheel or the zoom buttons. Hover a room, junction or corridor to see details.')),
  h('div', { class: 'section-title overlay-title' }, h('h2', {}, 'What to show'), help('Layers help you understand how the generator built the map. Turn them on one at a time to compare without hiding the rooms.')),
  overlayList,
  experiments,
  h('div', { class: 'section-title parameter-title' }, h('h2', {}, 'Parameters'), help('Changes are applied to the map automatically. Each field has an explanation and recommended limits.')),
  form,
  performanceGroup,
)
left.querySelector('.seed-control .icon-button:nth-of-type(1)').addEventListener('click', () => seedStep(-1))
left.querySelector('.seed-control .icon-button:nth-of-type(2)').addEventListener('click', () => seedStep(1))
left.querySelector('.seed-control .secondary-button').addEventListener('click', newSeed)

/* ---------------- Tools over the map ---------------- */

const zoomBy = (factor) => {
  const next = Math.max(0.3, Math.min(20, view.zoom * factor))
  const ratio = next / view.zoom
  view.x *= ratio
  view.y *= ratio
  view.zoom = next
  draw()
}
const resetView = () => { view.zoom = 1; view.x = 0; view.y = 0; draw() }

const toolbar = h('div', { class: 'map-toolbar', role: 'toolbar', 'aria-label': 'Map controls' })
const zoomOutButton = h('button', { type: 'button', class: 'map-tool-button', 'aria-label': 'Zoom out', title: 'Zoom out' }, '−')
const zoomInButton = h('button', { type: 'button', class: 'map-tool-button', 'aria-label': 'Zoom in', title: 'Zoom in' }, '+')
const fitButton = h('button', { type: 'button', class: 'map-tool-button fit-button' }, 'Fit map')
zoomOutButton.addEventListener('click', () => zoomBy(0.8))
zoomInButton.addEventListener('click', () => zoomBy(1.25))
fitButton.addEventListener('click', resetView)
toolbar.append(zoomOutButton, zoomLabel, zoomInButton, fitButton)
viewSection.append(toolbar, h('div', { class: 'map-instructions', id: 'map-instructions' }, 'Drag to move · wheel to zoom · hover elements for details'))
canvas.setAttribute('aria-describedby', 'map-instructions')

/* ---------------- Layout and generation ---------------- */

const scheduleRegenerate = () => {
  clearTimeout(regenTimer)
  updateLevelButtons()
  window.clearTimeout(scheduleRegenerate.indicatorTimer)
  changedStatus.dataset.pending = 'true'
  window.setTimeout(() => { delete changedStatus.dataset.pending }, 180)
  regenTimer = window.setTimeout(regenerate, 80)
}

const regenerate = () => {
  try {
    current = JSON.parse(stage_json(state.seed >>> 0, JSON.stringify(state.P)))
    hideHelp()
    draw()
    renderPanel()
  } catch (error) {
    current = null
    right.replaceChildren(h('div', { class: 'validity-card invalid' }, h('strong', {}, 'Could not generate the map'), h('p', {}, String(error))))
    draw()
  }
}

function setLevel(level) {
  state.level = level
  state.P = JSON.parse(default_params(level))
  state.defaults = clone(state.P)
  updateLevelButtons()
  buildForm()
  updateChangedIndicator()
  scheduleRegenerate()
}

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
  if (!drag) {
    showMapTooltip(event)
    return
  }
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

window.addEventListener('resize', draw)
window.addEventListener('keydown', (event) => {
  if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement || event.target?.isContentEditable) return
  if (event.key >= '1' && event.key <= '5') setLevel(Number(event.key))
  else if (event.key === 'r' || event.key === 'R') newSeed()
  else if (event.key === 'ArrowLeft') seedStep(-1)
  else if (event.key === 'ArrowRight') seedStep(1)
})

document.getElementById('app').replaceChildren(left, viewSection, right, toast)
updateLevelButtons()
buildForm()
updateChangedIndicator()
regenerate()

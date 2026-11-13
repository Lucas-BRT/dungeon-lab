// The walk: a fixed three.js scene behind the landing page. Scrolling carries a camera along the main path of one real floor, the one the
// generator draws for the seed and level in `data-seed` and `data-level`, built here in the browser by the same wasm module the lab uses.
// Scrolling stays native: this file only reads the scroll position. Without WebGL2, scripts or consent to data, the page keeps the
// server-rendered SVG of the same floor behind it. Geometry comes from build.js (pure data); this file owns everything that touches the GPU.

const root = document.querySelector('.walk-backdrop[data-three]')
const sceneMode = root?.dataset.mode ?? 'landing'

const params = new URLSearchParams(location.search)
const reduced = () => matchMedia('(prefers-reduced-motion: reduce)').matches || params.get('motion') === 'reduce'
const saveData = () => navigator.connection?.saveData === true

/** Colours come from the page's own tokens, so CSS stays the one place that says what the stone and the torches look like. */
const css = (name) => getComputedStyle(document.documentElement).getPropertyValue(name).trim()
const clamp = (x, lo, hi) => (x < lo ? lo : x > hi ? hi : x)
const smooth = (a, b, x) => { const t = clamp((x - a) / (b - a), 0, 1); return t * t * (3 - 2 * t) }
const lerp = (a, b, t) => a + (b - a) * t
const centreOfBounds = ([x0, y0, x1, y1]) => [(x0 + x1) / 2, (y0 + y1) / 2]

/** A number in [0, 1) from an integer, with no randomness: every visitor sees the same flicker. */
const hash = (n) => (Math.imul(n + 1, 2654435761) >>> 0) / 4294967296

function hasWebGL2() {
  try {
    return !!document.createElement('canvas').getContext('webgl2')
  } catch {
    return false
  }
}

function remember(key, value, store = localStorage) {
  try {
    if (value === undefined) return store.getItem(key)
    store.setItem(key, value)
  } catch {
    return null
  }
}

/* ---------------- Light, flames ---------------- */

const FLAME_VERTEX = `
  attribute float phase;
  attribute float lit;
  uniform float uTime;
  uniform float uSway;
  varying vec2 vUv;
  varying float vLevel;
  void main() {
    vUv = uv;
    vec4 mv = modelViewMatrix * instanceMatrix * vec4(0.0, 0.0, 0.0, 1.0);
    float n = (sin(uTime * 7.1 + phase * 6.2831) + sin(uTime * 11.3 + phase * 17.0) + sin(uTime * 17.9 + phase * 31.0)) / 3.0;
    vLevel = lit * (0.86 + 0.14 * n * uSway);
    mv.xy += position.xy * vec2(11.0, 20.0) * (0.9 + 0.1 * vLevel);
    mv.y += 9.0;
    gl_Position = projectionMatrix * mv;
  }
`
const FLAME_FRAGMENT = `
  uniform vec3 uColor;
  varying vec2 vUv;
  varying float vLevel;
  void main() {
    vec2 p = vUv - vec2(0.5, 0.32);
    p.x *= 1.7;
    float d = length(p) * 2.0;
    float a = smoothstep(1.0, 0.0, d);
    a *= a;
    vec3 core = mix(uColor, vec3(1.0, 0.86, 0.62), smoothstep(0.5, 0.0, d));
    gl_FragColor = vec4(core * a * vLevel, a * vLevel);
  }
`

/* ---------------- The floor texture ---------------- */

/** One canvas: flagstone joints along the floor's grid, each room tinted toward its shape colour, the mask as alpha. */
function floorTexture(THREE, floor, scene, size) {
  const [x0, y0, x1, y1] = scene.bounds
  const W = size
  const H = Math.round((size * (y1 - y0)) / (x1 - x0))
  const sx = W / (x1 - x0)
  const sy = H / (y1 - y0)
  const canvas = document.createElement('canvas')
  canvas.width = W
  canvas.height = H
  const c = canvas.getContext('2d')
  const m = scene.mask
  const small = document.createElement('canvas')
  small.width = m.nx
  small.height = m.ny
  const image = small.getContext('2d').createImageData(m.nx, m.ny)
  for (let i = 0; i < m.data.length; i++) { image.data[i * 4] = image.data[i * 4 + 1] = image.data[i * 4 + 2] = 255; image.data[i * 4 + 3] = m.data[i] }
  small.getContext('2d').putImageData(image, 0, 0)
  c.imageSmoothingEnabled = true
  c.drawImage(small, (m.ox - x0 + m.cell / 2) * sx, (m.oy - y0 + m.cell / 2) * sy, m.nx * m.cell * sx, m.ny * m.cell * sy)
  c.globalCompositeOperation = 'source-in'
  c.fillStyle = '#7a756d'
  c.fillRect(0, 0, W, H)
  c.globalCompositeOperation = 'source-atop'
  for (const room of floor.rooms) {
    if (!room.alive) continue
    const s = room.shape
    c.beginPath()
    if (s.kind === 'disc') c.arc((s.c[0] - x0) * sx, (s.c[1] - y0) * sy, s.r * sx, 0, Math.PI * 2)
    else s.pts.forEach(([x, y], i) => (i ? c.lineTo((x - x0) * sx, (y - y0) * sy) : c.moveTo((x - x0) * sx, (y - y0) * sy)))
    c.closePath()
    c.globalAlpha = 0.1
    c.fillStyle = css(`--map-${room.cls}`) || '#888'
    c.fill()
  }
  c.globalAlpha = 0.5
  c.strokeStyle = '#2e2a25'
  c.lineWidth = 1.5
  const joints = (list, vertical) => {
    for (let i = 0; i < list.length; i++) {
      const lo = list[i]
      const hi = list[i + 1] ?? lo
      for (const at of [lo, (lo + hi) / 2]) {
        c.beginPath()
        if (vertical) { c.moveTo((at - x0) * sx, 0); c.lineTo((at - x0) * sx, H) } else { c.moveTo(0, (at - y0) * sy); c.lineTo(W, (at - y0) * sy) }
        c.stroke()
      }
    }
  }
  joints(floor.gridX, true)
  joints(floor.gridY, false)
  c.globalAlpha = 1
  const texture = new THREE.CanvasTexture(canvas)
  texture.colorSpace = THREE.SRGBColorSpace
  texture.anisotropy = 4
  return texture
}

/* ---------------- Mount ---------------- */

/**
 * Builds the scene for `floor` on `canvas` and returns a handle. `mode` is 'landing' (scroll-driven), later 'leaf' and 'orbit'.
 * `three` is the vendored module, `scene` the result of build.js.
 */
export function mount({ THREE, canvas, floor, scene: built, mode = 'landing', page }) {
  const abyss = new THREE.Color(css('--abyss') || '#0b0e12')
  const renderer = new THREE.WebGLRenderer({ canvas, antialias: devicePixelRatio < 1.5, powerPreference: 'low-power' })
  renderer.outputColorSpace = THREE.SRGBColorSpace
  renderer.toneMapping = THREE.ACESFilmicToneMapping
  renderer.toneMappingExposure = 1.15
  renderer.setClearColor(abyss, 1)

  const scene = new THREE.Scene()
  scene.background = abyss
  scene.fog = new THREE.FogExp2(abyss, 0.0016)
  const camera = new THREE.PerspectiveCamera(50, 1, 10, 4000)
  let controls = null

  /* Walls */
  const wallGeometry = new THREE.BufferGeometry()
  const w = built.walls
  wallGeometry.setAttribute('position', new THREE.BufferAttribute(w.positions, 3))
  wallGeometry.setAttribute('normal', new THREE.BufferAttribute(w.normals, 3))
  const colors = new Float32Array(w.shade.length * 3)
  for (let i = 0; i < w.shade.length; i++) colors.fill(w.shade[i], i * 3, i * 3 + 3)
  wallGeometry.setAttribute('color', new THREE.BufferAttribute(colors, 3))
  wallGeometry.setIndex(new THREE.BufferAttribute(w.indices, 1))
  const wallMaterial = new THREE.MeshLambertMaterial({ color: '#a19b90', vertexColors: true })
  scene.add(new THREE.Mesh(wallGeometry, wallMaterial))

  /* Floor: one plane under everything, the texture's alpha cuts it to the shape of the floor. */
  const [bx0, by0, bx1, by1] = built.bounds
  const texture = floorTexture(THREE, floor, built, 1024)
  const plane = new THREE.PlaneGeometry(bx1 - bx0, by1 - by0)
  const floorMaterial = new THREE.MeshLambertMaterial({ map: texture, alphaTest: 0.5 })
  const ground = new THREE.Mesh(plane, floorMaterial)
  ground.rotation.x = -Math.PI / 2
  ground.position.set((bx0 + bx1) / 2, 0, (by0 + by1) / 2)
  scene.add(ground)

  /* Corner pillars at the junctions hide the seams where corridors cross. */
  const pillars = built.pillars.length / 2
  let pillarMesh = null
  if (pillars) {
    pillarMesh = new THREE.InstancedMesh(new THREE.BoxGeometry(7, 60, 7), new THREE.MeshLambertMaterial({ color: '#80786c' }), pillars)
    const m = new THREE.Matrix4()
    for (let i = 0; i < pillars; i++) pillarMesh.setMatrixAt(i, m.makeTranslation(built.pillars[i * 2], 30, built.pillars[i * 2 + 1]))
    scene.add(pillarMesh)
  }

  /* The stairwell: a square cut into the floor, steps going down, and cold light rising from it. */
  const stairs = built.stairs
  const landingMode = mode === 'landing'
  const leafMode = mode === 'leaf'
  const orbitMode = mode === 'orbit'
  const side = Math.min(stairs.radius * 1.1, 110)
  const stepMaterial = new THREE.MeshLambertMaterial({ color: '#615b52' })
  const steps = []
  for (let k = 0; landingMode && k < 8; k++) {
    const step = new THREE.Mesh(new THREE.BoxGeometry(side, 20, side / 8), stepMaterial)
    step.position.set(stairs.center[0], -10 - k * 20, stairs.center[1] - side / 2 + (k + 0.5) * (side / 8))
    steps.push(step)
    scene.add(step)
  }
  const below = new THREE.PointLight(css('--light-below') || '#6d8fc4', 400, 380, 1)
  below.position.set(stairs.center[0], -90, stairs.center[1])
  if (landingMode) scene.add(below)

  /* Torches: brackets and flames (two draw calls), lit by a fixed pool of lights that follows the camera. */
  const count = built.torches.length / 4
  const brackets = new THREE.InstancedMesh(new THREE.BoxGeometry(4, 16, 4), new THREE.MeshLambertMaterial({ color: '#3a3430' }), count)
  const flames = new THREE.InstancedMesh(new THREE.PlaneGeometry(1, 1), null, count)
  const phase = new Float32Array(count)
  const lit = new Float32Array(count)
  const m4 = new THREE.Matrix4()
  const spots = []
  for (let i = 0; i < count; i++) {
    const x = built.torches[i * 4]
    const z = built.torches[i * 4 + 1]
    spots.push([x, 44, z])
    brackets.setMatrixAt(i, m4.makeTranslation(x, 34, z))
    flames.setMatrixAt(i, m4.makeTranslation(x, 44, z))
    phase[i] = hash(i)
  }
  flames.geometry.setAttribute('phase', new THREE.InstancedBufferAttribute(phase, 1))
  const litAttribute = new THREE.InstancedBufferAttribute(lit, 1)
  flames.geometry.setAttribute('lit', litAttribute)
  const flameUniforms = { uTime: { value: 0 }, uSway: { value: 1 }, uColor: { value: new THREE.Color(css('--light-torch') || '#ffbf85') } }
  flames.material = new THREE.ShaderMaterial({ vertexShader: FLAME_VERTEX, fragmentShader: FLAME_FRAGMENT, uniforms: flameUniforms, transparent: true, depthWrite: false, blending: THREE.AdditiveBlending })
  flames.frustumCulled = false
  scene.add(brackets, flames)
  // In the leaf chamber there is one torch, brought from the nearest wall into the room.
  const leafCenter = built.leafRoom ? built.leafRoom.center : centreOfBounds(built.bounds)
  let leafTorch = -1
  if (leafMode && count) {
    let best = Infinity
    spots.forEach(([x, , z], i) => { const d = (x - leafCenter[0]) ** 2 + (z - leafCenter[1]) ** 2; if (d < best) { best = d; leafTorch = i } })
    spots[leafTorch] = [leafCenter[0] + 42, 44, leafCenter[1] - 34]
    brackets.setMatrixAt(leafTorch, m4.makeTranslation(spots[leafTorch][0], 34, spots[leafTorch][2]))
    flames.setMatrixAt(leafTorch, m4.makeTranslation(spots[leafTorch][0], 44, spots[leafTorch][2]))
  }
  // Torches light up outwards from the entrance: delay by distance from the first station.
  const entrance = built.stations[0]
  let farthest = 1
  const delays = spots.map(([x, , z]) => { const d = Math.sqrt((x - entrance.x) ** 2 + (z - entrance.y) ** 2); farthest = Math.max(farthest, d); return d })
  for (let i = 0; i < count; i++) delays[i] = (delays[i] / farthest) * 0.8

  /* Lights. Warm torches against cool stone; the overview is lit like a model on a table, neutral and soft. */
  // Decay 1, not the physical 2: wide soft pools of light instead of hot spots on the nearest wall and black beyond.
  const TORCH = 120
  const CARRIED = 110
  const fill = new THREE.Color(css('--light-fill') || '#2b3442')
  const dayFill = new THREE.Color('#b4ad9f')
  const hemisphere = new THREE.HemisphereLight(fill, abyss, 1.1)
  scene.add(hemisphere)
  const carried = new THREE.PointLight(css('--light-carried') || '#ffd4ad', CARRIED, 600, 1)
  scene.add(carried)
  const pool = []
  const POOL = 6
  for (let i = 0; i < POOL; i++) {
    const light = new THREE.PointLight(css('--light-torch') || '#ffbf85', TORCH, 360, 1)
    pool.push({ light, torch: -1 })
    scene.add(light)
  }
  let poolSize = leafMode ? 1 : POOL
  // For the overview: torches spread over the whole floor (farthest-point picks from the entrance), so the map reads as a lit model.
  const spread = []
  if (count) {
    const far = new Float32Array(count).fill(Infinity)
    let pick = 0
    let bestD = Infinity
    spots.forEach(([x, , z], i) => { const d = (x - built.stations[0].x) ** 2 + (z - built.stations[0].y) ** 2; if (d < bestD) { bestD = d; pick = i } })
    while (spread.length < Math.min(POOL, count)) {
      spread.push(pick)
      let next = 0
      for (let i = 0; i < count; i++) {
        far[i] = Math.min(far[i], (spots[i][0] - spots[pick][0]) ** 2 + (spots[i][2] - spots[pick][2]) ** 2)
        if (far[i] > far[next]) next = i
      }
      pick = next
    }
  }

  /* The path */
  const pts = built.path.points
  const lens = built.path.lengths
  const total = built.path.total
  const pointAt = (u, out) => {
    const d = clamp(u, 0, 1) * total
    let lo = 0
    let hi = lens.length - 1
    while (hi - lo > 1) { const mid = (lo + hi) >> 1; if (lens[mid] <= d) lo = mid; else hi = mid }
    const span = lens[hi] - lens[lo] || 1
    const t = clamp((d - lens[lo]) / span, 0, 1)
    out[0] = lerp(pts[lo * 3], pts[hi * 3], t)
    out[2] = lerp(pts[lo * 3 + 2], pts[hi * 3 + 2], t)
    return out
  }
  const stationU = built.stations.map((s) => s.u)
  const ALTITUDE = [900, 380, 320, 290, 260, 240, 220]
  const altitude = (u) => {
    for (let i = 0; i < stationU.length - 1; i++) if (u <= stationU[i + 1]) return lerp(ALTITUDE[i], ALTITUDE[i + 1], smooth(stationU[i], stationU[i + 1], u))
    return ALTITUDE[ALTITUDE.length - 1]
  }
  const centre = [(bx0 + bx1) / 2, (by0 + by1) / 2]
  // Where each room's text stands, as a share of the width to move the picture away from it (+ right, - left). Matches landing.css.
  const SHIFT = [0.14, 0.16, 0, 0, -0.14, 0.16, -0.05]
  const shiftAt = (u) => {
    for (let i = 0; i < stationU.length - 1; i++) if (u <= stationU[i + 1]) return lerp(SHIFT[i], SHIFT[i + 1], smooth(stationU[i], stationU[i + 1], u))
    return SHIFT[SHIFT.length - 1]
  }
  const leafAngle = (t) => 0.5 + (stepped ? 0 : 0.14 * Math.sin(t * 0.35))
  const aheadU = 0.035

  /* The camera pose for walk position `u` and footer descent `dive` (0 to 1). */
  const here = [0, 0, 0]
  const ahead = [0, 0, 0]
  const pose = { px: 0, py: 0, pz: 0, lx: 0, ly: 0, lz: 0, bird: 1 }
  if (orbitMode) {
    // The lab's 3D view: turn the whole floor with the pointer or the arrow keys (when the view has the focus).
    const span = Math.max(bx1 - bx0, by1 - by0)
    controls = new THREE.OrbitControls(camera, canvas)
    controls.enableDamping = true
    controls.dampingFactor = 0.1
    controls.maxPolarAngle = Math.PI * 0.49
    controls.minDistance = 120
    controls.maxDistance = 2800
    controls.target.set(centre[0], 0, centre[1])
    camera.position.set(centre[0], span * 0.95, centre[1] + span * 0.62)
    controls.listenToKeyEvents(canvas)
    controls.update()
  }
  const poseOrbit = () => {
    pose.px = camera.position.x
    pose.py = camera.position.y
    pose.pz = camera.position.z
    pose.lx = controls.target.x
    pose.ly = controls.target.y
    pose.lz = controls.target.z
    pose.bird = 0
    here[0] = pose.lx
    here[2] = pose.lz
    return pose
  }
  const poseLeaf = (t) => {
    const a = leafAngle(t)
    pose.px = leafCenter[0] + Math.sin(a) * 175
    pose.py = 105
    pose.pz = leafCenter[1] + Math.cos(a) * 175
    pose.lx = leafCenter[0]
    pose.ly = 18
    pose.lz = leafCenter[1]
    pose.bird = 0
    here[0] = leafCenter[0]
    here[2] = leafCenter[1]
    return pose
  }
  const poseAt = (u, dive) => {
    pointAt(u, here)
    pointAt(Math.min(1, u + aheadU), ahead)
    const alt = altitude(u)
    const bird = 1 - smooth(0, stationU[1], u)
    const portrait = innerWidth < innerHeight * 0.8
    // Walking pose: above the path and a little behind, looking ahead along it.
    const back = alt * 0.7
    const wx = here[0]
    const wy = alt
    const wz = here[2] + back
    // Bird's-eye pose over the middle of the floor, so the floor reads as a map.
    const span = Math.max(bx1 - bx0, by1 - by0)
    const bxp = centre[0]
    const byp = span * (portrait ? 1.45 : 0.98)
    const bzp = centre[1] + span * 0.5
    pose.px = lerp(wx, bxp, bird)
    pose.py = lerp(wy, byp, bird)
    pose.pz = lerp(wz, bzp, bird)
    pose.lx = lerp(ahead[0], centre[0], bird)
    pose.ly = 0
    pose.lz = lerp(ahead[2], centre[1], bird)
    pose.bird = bird
    if (dive > 0) {
      // Down the stairwell and into the dark.
      const sx = stairs.center[0]
      const sz = stairs.center[1]
      pose.px = lerp(pose.px, sx, dive)
      pose.pz = lerp(pose.pz, sz + 30 * (1 - dive), dive)
      pose.py = lerp(pose.py, -70, dive)
      pose.lx = lerp(pose.lx, sx, dive)
      pose.lz = lerp(pose.lz, sz, dive)
      pose.ly = lerp(0, -260, dive)
    }
    return pose
  }

  /* Scroll to walk position */
  const sections = page ? [...page.querySelectorAll('[data-station]')] : []
  const footer = document.querySelector('.site-footer')
  let tops = []
  let heights = []
  const measure = () => {
    const base = scrollY
    tops = sections.map((s) => s.getBoundingClientRect().top + base)
    heights = sections.map((s) => s.getBoundingClientRect().height)
  }
  const target = () => {
    if (!sections.length) return { u: 0, dive: 0, station: 0 }
    const vh = innerHeight
    const mid = scrollY + vh * 0.5
    let i = 0
    for (let k = 0; k < sections.length; k++) if (tops[k] <= mid) i = k
    const f = clamp((mid - tops[i]) / (heights[i] || 1), 0, 1)
    const u = i >= stationU.length - 1 ? 1 : lerp(stationU[i], stationU[i + 1], smooth(0.35, 1, f))
    let dive = 0
    if (footer) {
      const rect = footer.getBoundingClientRect()
      dive = clamp((vh - rect.top) / Math.max(1, Math.min(rect.height, vh * 0.6)), 0, 1)
    }
    return { u, dive, station: f > 0.65 && i < stationU.length - 1 ? i + 1 : i }
  }

  /* State */
  let state = { u: 0, dive: 0, station: -1 }
  let want = { u: 0, dive: 0, station: 0 }
  let stepped = reduced()
  const paused = () => remember('tapiti.walk.paused') === '1'
  if (paused()) stepped = true
  // `?tier=0` (to 3) pins the quality tier, for screenshots on slow software renderers.
  const pinned = params.has('tier')
  let tier = pinned ? clamp(Number(params.get('tier')) || 0, 0, 3) : Number(remember('tapiti.walk.tier', undefined, sessionStorage)) || 0
  if (tier >= 2) poolSize = Math.min(poolSize, 3)
  if (tier >= 3) flames.visible = false
  let dpr = 1
  const applySize = () => {
    const rect = canvas.getBoundingClientRect()
    const cap = innerWidth < 800 ? 1.25 : 1.5
    dpr = tier >= 1 ? 1 : Math.min(devicePixelRatio || 1, cap)
    renderer.setPixelRatio(dpr)
    renderer.setSize(Math.max(1, Math.round(rect.width)), Math.max(1, Math.round(rect.height)), false)
    camera.aspect = rect.width / Math.max(1, rect.height)
    camera.updateProjectionMatrix()
  }
  let lastHeight = innerHeight
  let resizeTimer = 0
  const onResize = () => {
    clearTimeout(resizeTimer)
    resizeTimer = setTimeout(() => {
      stillDrawn = false
      // The mobile URL bar changes the height by a few percent while scrolling: not a resize.
      const heightOnly = Math.abs(innerHeight - lastHeight) / lastHeight < 0.15 && canvas.width === Math.round(canvas.getBoundingClientRect().width * dpr)
      lastHeight = innerHeight
      measure()
      if (!heightOnly) applySize()
      wake()
    }, 100)
  }

  const minimapSvg = document.querySelector('.minimap-floor svg')
  const you = document.querySelector('.minimap .you')
  const viewBox = minimapSvg ? minimapSvg.viewBox.baseVal : null
  const stops = [...document.querySelectorAll('.minimap .stop')]
  const ticks = [...document.querySelectorAll('.route-list a')]
  let currentStation = -1
  const markStation = (i) => {
    if (i === currentStation) return
    currentStation = i
    for (const [k, el] of stops.entries()) k === i ? el.setAttribute('aria-current', 'true') : el.removeAttribute('aria-current')
    for (const [k, el] of ticks.entries()) k === i ? el.setAttribute('aria-current', 'true') : el.removeAttribute('aria-current')
  }

  const ordered = new Array(count)
  const scratch = new Float32Array(count)
  let started = performance.now()
  let lastScroll = 0
  let frameId = 0
  let timer = 0
  let stopped = false
  let first = true
  const samples = []
  let sampleStart = 0
  let prev = performance.now()
  let stillDrawn = false

  const render = (now) => {
    const t = (now - started) / 1000
    const ignition = stepped ? 1 : null
    // Lights: the pool goes to the torches nearest to where the camera looks.
    const p = orbitMode ? poseOrbit() : leafMode ? poseLeaf(t) : poseAt(state.u, state.dive)
    if (!orbitMode) {
      camera.position.set(p.px, p.py, p.pz)
      camera.lookAt(p.lx, p.ly, p.lz)
      camera.fov = innerWidth < innerHeight * 0.8 ? lerp(50, 60, p.bird) : 50
      // On wide screens the text stands at one side: the picture moves to the other, without turning the camera.
      const w = canvas.width
      const h = canvas.height
      const shift = landingMode && innerWidth >= 1100 ? shiftAt(state.u) * (1 - state.dive) : 0
      if (shift) camera.setViewOffset(w, h, -shift * w, 0, w, h)
      else camera.clearViewOffset()
      camera.updateProjectionMatrix()
    }
    scene.fog.density = leafMode ? 0.0035 : orbitMode ? 0.0003 : lerp(0.0014, 0.0002, p.bird)
    hemisphere.intensity = leafMode ? 1 : orbitMode ? 2.2 : lerp(1.4, 2.6, p.bird)
    hemisphere.color.lerpColors(fill, dayFill, orbitMode ? 1 : p.bird)
    for (let i = 0; i < count; i++) scratch[i] = (spots[i][0] - p.lx) ** 2 + (spots[i][2] - p.lz) ** 2
    for (let i = 0; i < count; i++) ordered[i] = i
    ordered.sort((a, b) => scratch[a] - scratch[b])
    let changed = false
    for (let i = 0; i < count; i++) {
      const level = leafMode ? (i === leafTorch ? smooth(0, 0.6, t) : 0) : (ignition ?? smooth(0, 0.45, t - delays[i]))
      if (lit[i] !== level) { lit[i] = level; changed = true }
    }
    if (changed) litAttribute.needsUpdate = true
    flameUniforms.uTime.value = stepped ? 0 : t
    flameUniforms.uSway.value = stepped ? 0 : 1
    for (let k = 0; k < POOL; k++) {
      const slot = pool[k]
      if (k >= poolSize) { slot.light.intensity = 0; continue }
      const torch = !orbitMode && !leafMode && p.bird > 0.5 && spread[k] !== undefined ? spread[k] : ordered[k]
      const [x, y, z] = spots[torch]
      slot.light.position.set(x, y, z)
      const n = stepped ? 0 : (Math.sin(t * 7.1 + hash(torch) * 6.28) + Math.sin(t * 11.3 + hash(torch) * 17) + Math.sin(t * 17.9 + hash(torch) * 31)) / 3
      slot.light.intensity = TORCH * (1 + 2 * p.bird) * (0.86 + 0.14 * n) * lit[torch]
    }
    carried.position.set(p.px + (p.lx - p.px) * 0.15, Math.max(p.py - 40, -40), p.pz + (p.lz - p.pz) * 0.15)
    carried.intensity = (leafMode ? 0.5 : orbitMode ? 0.8 : 1) * CARRIED * smooth(0, 0.45, t) * (1 - p.bird * 0.7)
    below.intensity = landingMode ? 400 * smooth(0.6, 1, state.u) * (1 - state.dive * 0.4) : 0
    renderer.render(scene, camera)
    if (first) {
      first = false
      document.documentElement.classList.add('walk-on')
    }
    // The minimap's "you are here" dot and the current station.
    if (you && viewBox) {
      you.style.left = `${((here[0] - viewBox.x) / viewBox.width) * 100}%`
      you.style.top = `${((here[2] - viewBox.y) / viewBox.height) * 100}%`
    }
    markStation(want.station)
    window.__walk = { tier, calls: renderer.info.render.calls, triangles: renderer.info.render.triangles, stations: built.stations, camera: { x: p.px, y: p.py, z: p.pz, u: state.u }, torches: count, poolSize, lights: pool.length + 3, stepped }
  }

  const adapt = (now) => {
    const dt = now - prev
    prev = now
    if (stepped || pinned || dt > 200) return
    if (!sampleStart) sampleStart = now
    samples.push(dt)
    if (now - sampleStart >= 2000) {
      samples.sort((a, b) => a - b)
      const p95 = samples[Math.floor(samples.length * 0.95)]
      samples.length = 0
      sampleStart = 0
      if (p95 > 22 && tier < 3) {
        tier++
        remember('tapiti.walk.tier', String(tier), sessionStorage)
        if (tier === 1) applySize()
        else if (tier === 2) poolSize = Math.min(poolSize, 3)
        else flames.visible = false
      }
    }
  }

  const step = (now) => {
    frameId = 0
    timer = 0
    if (stopped || document.hidden || offscreen) return
    if (leafMode) {
      adapt(now)
      render(now)
      if (stepped) { stillDrawn = true; return }
      frameId = requestAnimationFrame(step)
      return
    }
    want = target()
    if (stepped) want.u = stationU[want.station]
    const k = stepped ? 1 : 1 - Math.exp(-(Math.min(now - prev, 100) / 1000) / 0.18)
    state.u += (want.u - state.u) * k
    state.dive += (want.dive - state.dive) * k
    const moving = Math.abs(want.u - state.u) > 1e-4 || Math.abs(want.dive - state.dive) > 1e-4
    if (!moving) { state.u = want.u; state.dive = want.dive }
    adapt(now)
    if (stepped && stillDrawn && !moving && state.station === want.station) return
    state.station = want.station
    render(now)
    const ignitionBusy = now - started < 1400
    if (stepped) { stillDrawn = true; return }
    if (moving || ignitionBusy || now - lastScroll < 300) { frameId = requestAnimationFrame(step); return }
    if (state.dive >= 1) return
    // Resting: the flames still flicker, at 15 fps, for a few seconds, then the loop sleeps until the next scroll.
    if (now - lastScroll < 6000) timer = setTimeout(() => { frameId = requestAnimationFrame(step) }, 66)
  }
  let lastTouch = 0
  const stepOrbit = (now) => {
    frameId = 0
    timer = 0
    if (stopped || document.hidden) return
    controls.update()
    adapt(now)
    render(now)
    // While the pointer or damping moves the camera, every frame; at rest, the flames at 15 fps.
    if (now - lastTouch < 900) frameId = requestAnimationFrame(stepOrbit)
    else timer = setTimeout(() => { frameId = requestAnimationFrame(stepOrbit) }, stepped ? 400 : 66)
  }
  const loop = orbitMode ? stepOrbit : step
  const wake = () => {
    if (stopped || frameId || timer) return
    frameId = requestAnimationFrame(loop)
  }
  if (orbitMode) controls.addEventListener('change', () => { lastTouch = performance.now(); wake() })
  const onScroll = () => { lastScroll = performance.now(); wake() }
  if (landingMode) addEventListener('scroll', onScroll, { passive: true })
  addEventListener('resize', onResize)
  document.addEventListener('visibilitychange', () => { if (!document.hidden) { prev = performance.now(); wake() } })
  canvas.addEventListener('webglcontextlost', (event) => {
    event.preventDefault()
    stopped = true
    document.documentElement.classList.remove('walk-on')
  })
  new ResizeObserver(onResize).observe(canvas)
  // On narrow screens the scene sits behind the entrance only and scrolls away: no frames while it is out of sight.
  let offscreen = false
  new IntersectionObserver(([entry]) => { offscreen = !entry.isIntersecting; if (!offscreen) wake() }).observe(canvas)

  measure()
  applySize()
  want = target()
  state = { u: stepped ? stationU[want.station] : 0, dive: stepped ? want.dive : 0, station: -1 }
  started = performance.now()
  prev = started
  lastScroll = started
  wake()

  const setStepped = (value) => {
    stepped = value
    remember('tapiti.walk.paused', value ? '1' : '0')
    stillDrawn = false
    wake()
  }
  return {
    get stepped() { return stepped },
    pause: () => setStepped(true),
    resume: () => setStepped(reduced()),
    /** `keepContext` keeps the WebGL context, for a rebuild on the same canvas (the lab does that on every regenerate). */
    dispose({ keepContext = false } = {}) {
      stopped = true
      clearTimeout(timer)
      cancelAnimationFrame(frameId)
      removeEventListener('scroll', onScroll)
      removeEventListener('resize', onResize)
      controls?.dispose()
      for (const o of [wallGeometry, plane, texture, wallMaterial, floorMaterial, stepMaterial]) o.dispose()
      renderer.dispose()
      if (!keepContext) renderer.forceContextLoss()
      window.__walk = null
    },
  }
}

/* ---------------- The landing page ---------------- */

async function boot() {
  if (!root || params.get('scene') === 'off' || saveData() || !hasWebGL2()) return
  const canvas = root.querySelector('canvas')
  const seed = Number(root.dataset.seed)
  const level = Number(root.dataset.level)
  const [wasm, build, THREE] = await Promise.all([import('/lab/pkg/tapiti.js'), import(root.dataset.build), import(root.dataset.three)])
  await wasm.default()
  const answer = JSON.parse(wasm.stage_json(seed, wasm.default_params(level)))
  const floor = answer.floor
  const built = build.build(floor)
  const handle = mount({ THREE, canvas, floor, scene: built, mode: sceneMode, page: document.querySelector('.rooms') })

  const toggle = document.querySelector('.route-pause')
  if (toggle) {
    toggle.hidden = false
    const label = () => {
      const on = handle.stepped
      toggle.setAttribute('aria-pressed', String(on))
      toggle.textContent = on ? toggle.dataset.resume : toggle.dataset.pause
    }
    label()
    toggle.addEventListener('click', () => { handle.stepped ? handle.resume() : handle.pause(); label() })
  }
  addEventListener('pagehide', (event) => { if (!event.persisted) handle.dispose() })
}

if (root) {
  const start = () => boot().catch((error) => console.warn('walk: the scene did not start, the map stays', error))
  const idle = () => (window.requestIdleCallback ? requestIdleCallback(start, { timeout: 2000 }) : setTimeout(start, 200))
  document.readyState === 'complete' ? idle() : addEventListener('load', idle, { once: true })
}

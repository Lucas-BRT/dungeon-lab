// Builds apps/site/assets/vendor/three-subset.min.js: the parts of three.js the walk scene uses, tree-shaken into one ES module.
//   node scripts/vendor-three.mjs        (run by hand; needs Node 22+, npm and network; the result is committed)
// Why a bundle: since r171 three's `three.module.js` imports `./three.core.js`, and a hashed Topcoat asset cannot resolve a relative
// sibling import. Why committed: CI's test job and the Docker site stage stay offline and Node-free, and `asset!` needs the file at
// compile time. Bump THREE only to a release at least two weeks old.
import { execFileSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import { mkdtempSync, readFileSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'
import { gzipSync } from 'node:zlib'

const THREE = '0.186.1'
const ESBUILD = '0.28.2'
const BUDGET_GZIP = 160 * 1024
const out = resolve(dirname(fileURLToPath(import.meta.url)), '../apps/site/assets/vendor/three-subset.min.js')

const names = `WebGLRenderer Scene PerspectiveCamera FogExp2 Color BufferGeometry BufferAttribute InstancedBufferAttribute Mesh InstancedMesh
  MeshLambertMaterial MeshBasicMaterial ShaderMaterial PointLight HemisphereLight CanvasTexture CatmullRomCurve3 Vector2 Vector3 Matrix4
  Quaternion Euler Object3D Group MathUtils PlaneGeometry BoxGeometry SRGBColorSpace ACESFilmicToneMapping AdditiveBlending DoubleSide
  FrontSide RepeatWrapping ClampToEdgeWrapping LinearFilter LinearMipmapLinearFilter`.split(/\s+/)

const dir = mkdtempSync(join(tmpdir(), 'tapiti-three-'))
const run = (cmd, args) => execFileSync(cmd, args, { cwd: dir, stdio: ['ignore', 'pipe', 'inherit'] }).toString()
writeFileSync(join(dir, 'package.json'), '{"name":"tmp","private":true,"type":"module"}')
run('npm', ['install', '--no-audit', '--no-fund', `three@${THREE}`, `esbuild@${ESBUILD}`])
writeFileSync(join(dir, 'entry.js'), [
  `export { ${names.join(', ')} } from 'three'`,
  `export { OrbitControls } from 'three/addons/controls/OrbitControls.js'`,
  `export { mergeGeometries } from 'three/addons/utils/BufferGeometryUtils.js'`,
].join('\n'))
run(join(dir, 'node_modules/.bin/esbuild'), ['entry.js', '--bundle', '--format=esm', '--minify', '--legal-comments=eof', '--target=es2022', '--outfile=bundle.js'])

const bundle = readFileSync(join(dir, 'bundle.js'), 'utf8')
const header = `/* three@${THREE} subset (MIT), bundled by scripts/vendor-three.mjs with esbuild@${ESBUILD}. Do not edit. */\n`
const text = header + bundle
writeFileSync(out, text)
const gzip = gzipSync(text, { level: 9 }).length
const sha = createHash('sha256').update(text).digest('hex')
console.log(`${out}\n  ${text.length} bytes, ${gzip} bytes gzip -9, sha256 ${sha}`)
if (gzip > BUDGET_GZIP) { console.error(`over the gzip budget of ${BUDGET_GZIP} bytes`); process.exit(1) }

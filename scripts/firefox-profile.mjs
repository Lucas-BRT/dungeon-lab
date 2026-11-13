// A fresh Firefox profile for the headless checks. A CI runner has no GPU, so WebGL2 (the landing walk, the lab's 3D view) is allowed on
// Mesa's software rasterizer: Firefox blocks software renderers by default.
import { mkdtempSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

const PREFS = [
  ['webgl.disabled', false],
  ['webgl.enable-webgl2', true],
  ['webgl.force-enabled', true],
  ['webgl.forbid-software', false],
  ['gfx.webrender.software', true],
]

export const freshProfile = (name) => {
  const dir = mkdtempSync(join(tmpdir(), `tapiti-${name}-`))
  writeFileSync(join(dir, 'user.js'), PREFS.map(([key, value]) => `user_pref(${JSON.stringify(key)}, ${value});\n`).join(''))
  return dir
}

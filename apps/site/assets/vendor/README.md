# vendor

`three-subset.min.js` is the part of [three.js](https://threejs.org) `0.186.1` that the walk scene uses (renderer, scene graph, lights,
instancing, a few materials and geometries, `CatmullRomCurve3`, `OrbitControls`, `mergeGeometries`), tree-shaken into one ES module.
three.js is MIT licensed; the notice is kept at the end of the file.

- Built by `node scripts/vendor-three.mjs` (esbuild `0.28.2`, `--minify`, `--target=es2022`), by hand, with network. The result is committed so
  CI and the Docker image stay offline and Node-free, and so `asset!` finds the file at compile time.
- Why a bundle: since r171 three's own `three.module.js` imports `./three.core.js`, and a hashed Topcoat asset cannot resolve a relative
  sibling import.
- Size: 569,583 bytes, 143,002 bytes gzip -9 (budget 160 KB gzip, enforced by the script).
- sha256: `fd548321910638d864917ff67a4f52a90cddec0a6d556ed167597beb73cb2000`.
- To add an export, add its name to the list in the script and run it again. Bump the version only to a release at least two weeks old.

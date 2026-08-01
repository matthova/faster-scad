// Headless render in Node (ESM). Run with: node node-render.mjs
//
// The Node build instantiates the wasm on import, so `render()` works with no
// init step. Mesh data comes back as a non-indexed triangle soup (9 f32 per
// triangle in both `positions` and `normals`).
import { render, version } from "quito-engine";

console.log("quito-engine", await version());

const r = await render("difference() { cube(20, center=true); sphere(12); }");
if (!r.ok) {
  console.error(r.error);
  process.exit(1);
}

console.log(
  `${r.triangleCount} triangles, ${r.vertexCount} verts, ` +
    `volume ${r.volume.toFixed(2)}, area ${r.area.toFixed(2)}`,
);

// The Quito preview webview: a three.js orbit viewer for geometry rendered by
// the language server. This surface is a *dumb display* — it does not evaluate
// OpenSCAD. The quito-lsp server computes geometry on the native kernel and
// pushes it here; we just draw the vertex buffers.
//
// Message protocol with the extension host (see preview.ts):
//   1. this script loads   → postMessage {type:'ready'}
//   2. host streams meshes  → {type:'mesh', positions, normals, ...} (base64)
//                           → {type:'error', message}
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

declare function acquireVsCodeApi(): { postMessage: (msg: unknown) => void };
const vscode = acquireVsCodeApi();

const canvas = document.getElementById("canvas") as HTMLCanvasElement;
const statusEl = document.getElementById("status") as HTMLDivElement;

function setStatus(text: string, isError = false): void {
  statusEl.textContent = text;
  statusEl.classList.toggle("error", isError);
}

/** Decode base64 (raw little-endian f32 bytes) into a `Float32Array`. */
function b64ToF32(b64: string): Float32Array {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) {
    bytes[i] = bin.charCodeAt(i);
  }
  return new Float32Array(bytes.buffer);
}

// ---- three.js scene ----------------------------------------------------------
const renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
renderer.setPixelRatio(window.devicePixelRatio);

const scene = new THREE.Scene();
scene.background = new THREE.Color(0x1e1e1e);

const camera = new THREE.PerspectiveCamera(45, 1, 0.01, 100000);
camera.up.set(0, 0, 1); // Z-up, like OpenSCAD
camera.position.set(60, -80, 50);

const controls = new OrbitControls(camera, renderer.domElement);
controls.enableDamping = true;

const key = new THREE.DirectionalLight(0xffffff, 2.2);
key.position.set(40, -60, 80);
scene.add(key);
const fill = new THREE.DirectionalLight(0xffffff, 0.7);
fill.position.set(-50, 40, 20);
scene.add(fill);
scene.add(new THREE.AmbientLight(0xffffff, 0.5));

const grid = new THREE.GridHelper(200, 20, 0x3a3f4b, 0x2a2e37);
grid.rotation.x = Math.PI / 2; // grid in XY plane
scene.add(grid);
scene.add(new THREE.AxesHelper(20));

const material = new THREE.MeshStandardMaterial({
  color: 0xf5c542,
  metalness: 0.1,
  roughness: 0.6,
  flatShading: true,
  side: THREE.DoubleSide,
});
let mesh: THREE.Mesh | null = null;
/** Per-group materials for a colored render, tracked so they can be disposed. */
let groupMaterials: THREE.Material[] = [];
let hasFramed = false;

/** A colored preview group: a vertex range into the soup + its color and mode. */
interface PreviewGroup {
  start: number;
  count: number;
  color: [number, number, number, number];
  mode: "solid" | "highlight" | "background";
}

function resize(): void {
  const w = canvas.clientWidth;
  const h = canvas.clientHeight;
  if (w === 0 || h === 0) {
    return;
  }
  renderer.setSize(w, h, false);
  camera.aspect = w / h;
  camera.updateProjectionMatrix();
}
window.addEventListener("resize", resize);
resize();

function animate(): void {
  requestAnimationFrame(animate);
  controls.update();
  renderer.render(scene, camera);
}
animate();

function clearMesh(): void {
  if (mesh) {
    scene.remove(mesh);
    mesh.geometry.dispose();
    mesh = null;
  }
  for (const m of groupMaterials) {
    m.dispose();
  }
  groupMaterials = [];
}

function buildGeom(positions: Float32Array, normals: Float32Array): THREE.BufferGeometry {
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geo.setAttribute("normal", new THREE.BufferAttribute(normals, 3));
  geo.computeBoundingBox();
  return geo;
}

/** Frame the model the first time we see one. */
function frame(geo: THREE.BufferGeometry): void {
  if (hasFramed || !geo.boundingBox) {
    return;
  }
  const box = geo.boundingBox;
  const size = new THREE.Vector3();
  const center = new THREE.Vector3();
  box.getSize(size);
  box.getCenter(center);
  const radius = Math.max(size.x, size.y, size.z, 1) * 1.4;
  controls.target.copy(center);
  camera.position.set(center.x + radius, center.y - radius * 1.3, center.z + radius);
  camera.near = radius / 100;
  camera.far = radius * 100;
  camera.updateProjectionMatrix();
  hasFramed = true;
}

function setMesh(positions: Float32Array, normals: Float32Array): void {
  clearMesh();
  if (positions.length === 0) {
    return;
  }
  const geo = buildGeom(positions, normals);
  mesh = new THREE.Mesh(geo, material);
  scene.add(mesh);
  frame(geo);
}

/** The three.js material for a preview group: `#` → translucent red, `%` →
 *  translucent gray, solid → the group color (transparent when alpha < 1). */
function materialForGroup(g: PreviewGroup): THREE.Material {
  const base = { flatShading: true, side: THREE.DoubleSide as THREE.Side };
  if (g.mode === "highlight") {
    return new THREE.MeshStandardMaterial({ ...base, color: 0xff3b30, transparent: true, opacity: 0.5 });
  }
  if (g.mode === "background") {
    return new THREE.MeshStandardMaterial({ ...base, color: 0x888888, transparent: true, opacity: 0.3 });
  }
  const [r, gr, b, a] = g.color;
  return new THREE.MeshStandardMaterial({
    ...base,
    color: new THREE.Color(r, gr, b),
    metalness: 0.1,
    roughness: 0.6,
    transparent: a < 1,
    opacity: a,
  });
}

/** Render `color()`/`#`/`%` groups: one geometry, one material per group. */
function setColoredMesh(
  positions: Float32Array,
  normals: Float32Array,
  groups: PreviewGroup[]
): void {
  clearMesh();
  if (positions.length === 0 || groups.length === 0) {
    setMesh(positions, normals);
    return;
  }
  const geo = buildGeom(positions, normals);
  groupMaterials = groups.map((g, i) => {
    geo.addGroup(g.start, g.count, i);
    return materialForGroup(g);
  });
  mesh = new THREE.Mesh(geo, groupMaterials);
  scene.add(mesh);
  frame(geo);
}

// ---- message loop ------------------------------------------------------------
interface MeshMsg {
  type: "mesh";
  positions: string;
  normals: string;
  previewPositions?: string;
  previewNormals?: string;
  groups?: PreviewGroup[];
  triangleCount: number;
  vertexCount: number;
  volume: number;
  area: number;
}
interface ErrorMsg {
  type: "error";
  message: string;
}

window.addEventListener("message", (event: MessageEvent) => {
  const msg = event.data as MeshMsg | ErrorMsg | undefined;
  switch (msg?.type) {
    case "mesh":
      // Colored channel when the model uses color()/#/%, else the plain mesh.
      if (msg.groups && msg.groups.length > 0 && msg.previewPositions) {
        setColoredMesh(
          b64ToF32(msg.previewPositions),
          b64ToF32(msg.previewNormals ?? ""),
          msg.groups
        );
      } else {
        setMesh(b64ToF32(msg.positions), b64ToF32(msg.normals));
      }
      setStatus(
        `${msg.triangleCount.toLocaleString()} triangles · volume ${msg.volume.toFixed(2)} · area ${msg.area.toFixed(2)}`
      );
      break;
    case "error":
      setStatus(msg.message || "render error", true);
      break;
  }
});

// Announce we're ready so the host starts a server-side preview.
vscode.postMessage({ type: "ready" });

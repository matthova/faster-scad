// The Quito preview webview: a three.js orbit viewer driven by the wasm engine.
//
// Boot handshake with the extension host (see preview.ts):
//   1. this script loads      → postMessage {type:'loaded'}
//   2. host replies           → {type:'init', engineJs, wasmUri}
//   3. we import + init wasm   → postMessage {type:'ready'}
//   4. host streams source     → {type:'render', source}
//
// The engine glue is imported dynamically from a webview resource URI, so
// esbuild does not try to bundle it.
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

// Minimal shape of the wasm module we use.
interface RenderResult {
  ok: boolean;
  error: string;
  positions: Float32Array;
  normals: Float32Array;
  triangle_count: number;
  volume: number;
  area: number;
}
interface Engine {
  default: (wasm: string) => Promise<unknown>;
  start?: () => void;
  render_with_files: (
    source: string,
    names: string[],
    values: string[],
    fileNames: string[],
    fileContents: string[]
  ) => RenderResult;
}

declare function acquireVsCodeApi(): { postMessage: (msg: unknown) => void };
const vscode = acquireVsCodeApi();

const canvas = document.getElementById("canvas") as HTMLCanvasElement;
const statusEl = document.getElementById("status") as HTMLDivElement;

function setStatus(text: string, isError = false): void {
  statusEl.textContent = text;
  statusEl.classList.toggle("error", isError);
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
let hasFramed = false;

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

function setMesh(positions: Float32Array, normals: Float32Array): void {
  if (mesh) {
    scene.remove(mesh);
    mesh.geometry.dispose();
    mesh = null;
  }
  if (positions.length === 0) {
    return;
  }
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.BufferAttribute(positions, 3));
  geo.setAttribute("normal", new THREE.BufferAttribute(normals, 3));
  geo.computeBoundingBox();
  mesh = new THREE.Mesh(geo, material);
  scene.add(mesh);

  // Frame the model the first time we see one.
  if (!hasFramed && geo.boundingBox) {
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
}

// ---- engine + message loop ---------------------------------------------------
let engine: Engine | undefined;

async function initEngine(engineJs: string, wasmUri: string): Promise<void> {
  try {
    const mod = (await import(/* @vite-ignore */ engineJs)) as Engine;
    await mod.default(wasmUri);
    mod.start?.();
    engine = mod;
    setStatus("Ready.");
    vscode.postMessage({ type: "ready" });
  } catch (err) {
    setStatus(`Failed to load engine: ${err}`, true);
  }
}

function doRender(source: string): void {
  if (!engine) {
    return;
  }
  try {
    const res = engine.render_with_files(source, [], [], [], []);
    if (!res.ok) {
      setStatus(res.error || "render error", true);
      return;
    }
    setMesh(res.positions, res.normals);
    setStatus(
      `${res.triangle_count.toLocaleString()} triangles · volume ${res.volume.toFixed(2)} · area ${res.area.toFixed(2)}`
    );
  } catch (err) {
    setStatus(`render crashed: ${err}`, true);
  }
}

window.addEventListener("message", (event: MessageEvent) => {
  const msg = event.data;
  switch (msg?.type) {
    case "init":
      void initEngine(msg.engineJs, msg.wasmUri);
      break;
    case "render":
      doRender(msg.source);
      break;
  }
});

// Announce we're loaded so the host sends engine URIs.
vscode.postMessage({ type: "loaded" });

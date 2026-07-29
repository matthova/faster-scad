// A minimal three.js orbit viewer with a grid, axes, and flat-shaded mesh.
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

/** Model bounding-box size (mm) reported after each render. */
export interface MeshInfo {
  x: number;
  y: number;
  z: number;
}

/** A named camera orientation. */
export type ViewPreset = "iso" | "front" | "back" | "top" | "bottom" | "right" | "left";

/** Light/dark appearance, following the OS `prefers-color-scheme`. */
export type ThemeMode = "light" | "dark";

/** Camera projection: `perspective` (foreshortened) or `orthographic` (parallel,
 *  so an iso preset renders as a true isometric view). */
export type Projection = "perspective" | "orthographic";

/** A colored preview group: a triangle range (vertex offsets) + color + mode. */
export interface PreviewGroup {
  start: number;
  count: number;
  color: [number, number, number, number];
  mode: "solid" | "highlight" | "background";
}

/** A source byte-span `[start, end]` (into the main document). */
export type Span = [number, number];

/** A provenance group: a vertex range (offsets into the provenance soup) tagged
 *  with the source span that produced it (`null` when unattributable). */
export interface ProvenanceGroup {
  start: number;
  count: number;
  span: Span | null;
}

/** Overlay material for the code→model highlight: a bright wash drawn on top of
 *  the model (depthTest off) so the selected geometry reads clearly regardless of
 *  the model's own per-group colors. */
const HIGHLIGHT_MATERIAL = new THREE.MeshBasicMaterial({
  color: 0x4fc3f7,
  transparent: true,
  opacity: 0.45,
  depthTest: false,
  depthWrite: false,
  side: THREE.DoubleSide,
});

/** Material for the off-scene pick mesh. Never rendered — only `raycast` reads
 *  it, for `material.side`. `DoubleSide` so 2D flat meshes (whose triangles all
 *  face +z) stay pickable when the plane is viewed from below (e.g. the `bottom`
 *  preset or orbiting under z=0). */
const PICK_MATERIAL = new THREE.MeshBasicMaterial({ side: THREE.DoubleSide });

export class Viewer {
  private renderer: THREE.WebGLRenderer;
  private scene: THREE.Scene;
  private perspCamera: THREE.PerspectiveCamera;
  private orthoCamera: THREE.OrthographicCamera;
  private camera: THREE.PerspectiveCamera | THREE.OrthographicCamera;
  private controls: OrbitControls;
  private projection: Projection = "perspective";
  private changeListeners = new Set<() => void>();
  private mesh: THREE.Mesh | null = null;
  private geometry: THREE.BufferGeometry | null = null;
  private materials: THREE.Material[] = [];
  private hasFramed = false;
  private preset: ViewPreset = "iso";
  /** The floor grid; rebuilt on theme change (GridHelper colors are fixed at
   *  construction), so kept as a ref to remove/dispose the old one. */
  private grid: THREE.GridHelper | null = null;

  // ---- provenance picking / highlighting ----
  private raycaster = new THREE.Raycaster();
  private pointer = new THREE.Vector2();
  /** Off-scene mesh built from the provenance soup, raycast for model→code. */
  private pickMesh: THREE.Mesh | null = null;
  private pickGeometry: THREE.BufferGeometry | null = null;
  private provPositions: Float32Array = new Float32Array(0);
  private provNormals: Float32Array = new Float32Array(0);
  private provGroups: ProvenanceGroup[] = [];
  /** Overlay mesh showing the code→model highlight (added to the scene). */
  private highlightMesh: THREE.Mesh | null = null;
  private onPickCb: ((span: Span | null) => void) | null = null;
  /** Pointer-down screen position, to tell a click from an orbit drag. */
  private downPos: { x: number; y: number } | null = null;

  constructor(
    canvas: HTMLCanvasElement,
    private onInfo?: (info: MeshInfo | null) => void,
  ) {
    // preserveDrawingBuffer lets us read the canvas back (Save PNG) any frame.
    this.renderer = new THREE.WebGLRenderer({
      canvas,
      antialias: true,
      preserveDrawingBuffer: true,
    });
    this.renderer.setPixelRatio(window.devicePixelRatio);

    this.scene = new THREE.Scene();

    this.perspCamera = new THREE.PerspectiveCamera(45, 1, 0.01, 100000);
    this.perspCamera.up.set(0, 0, 1); // Z-up, like OpenSCAD
    this.perspCamera.position.set(60, -80, 50);

    // The orthographic camera shadows the perspective one; its frustum is
    // recomputed on framing/resize/toggle so both show the same extent.
    this.orthoCamera = new THREE.OrthographicCamera(-1, 1, 1, -1, 0.01, 100000);
    this.orthoCamera.up.set(0, 0, 1);
    this.orthoCamera.position.copy(this.perspCamera.position);

    this.camera = this.perspCamera;
    this.controls = this.makeControls();

    const key = new THREE.DirectionalLight(0xffffff, 2.2);
    key.position.set(40, -60, 80);
    this.scene.add(key);
    const fill = new THREE.DirectionalLight(0xffffff, 0.7);
    fill.position.set(-50, 40, 20);
    this.scene.add(fill);
    this.scene.add(new THREE.AmbientLight(0xffffff, 0.5));

    this.scene.add(new THREE.AxesHelper(20));
    // Background + grid colors follow the OS appearance; setTheme builds the
    // initial grid so the first frame already matches the OS.
    this.setTheme(
      window.matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light",
    );

    // Provenance picking: a click (not an orbit drag) selects the source
    // statement under the cursor. We arm on pointerdown and only fire on
    // pointerup if the pointer barely moved, so drags rotate/pan as usual.
    this.renderer.domElement.addEventListener("pointerdown", (e) => {
      if (e.button === 0) this.downPos = { x: e.clientX, y: e.clientY };
    });
    this.renderer.domElement.addEventListener("pointerup", (e) => {
      const d = this.downPos;
      this.downPos = null;
      if (!d || e.button !== 0) return;
      if (Math.hypot(e.clientX - d.x, e.clientY - d.y) > 4) return; // a drag
      this.pickAt(e.clientX, e.clientY);
    });

    this.resize();
    window.addEventListener("resize", () => this.resize());
    this.animate();
  }

  /** Bind fresh OrbitControls to the active camera, re-attaching any registered
   *  change listeners. Called on construction and whenever the projection (and
   *  thus the camera instance) changes. */
  private makeControls(): OrbitControls {
    const controls = new OrbitControls(this.camera, this.renderer.domElement);
    controls.enableDamping = true;
    for (const cb of this.changeListeners) controls.addEventListener("change", cb);
    return controls;
  }

  private animate = () => {
    requestAnimationFrame(this.animate);
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
  };

  resize() {
    const canvas = this.renderer.domElement;
    const w = canvas.clientWidth;
    const h = canvas.clientHeight;
    if (w === 0 || h === 0) return;
    this.renderer.setSize(w, h, false);
    const aspect = w / h;
    this.perspCamera.aspect = aspect;
    this.perspCamera.updateProjectionMatrix();
    // Keep the ortho frustum height fixed; rederive its width from the aspect.
    const halfH = (this.orthoCamera.top - this.orthoCamera.bottom) / 2;
    this.orthoCamera.left = -halfH * aspect;
    this.orthoCamera.right = halfH * aspect;
    this.orthoCamera.updateProjectionMatrix();
  }

  /** Apply the OS light/dark appearance to the scene: background color and the
   *  floor grid. The grid is rebuilt (its colors are fixed at construction) and
   *  the previous one disposed, so this is safe to call repeatedly on flips. */
  setTheme(mode: ThemeMode) {
    const dark = mode === "dark";
    this.scene.background = new THREE.Color(dark ? 0x1a1d23 : 0xf3f3f3);
    if (this.grid) {
      this.scene.remove(this.grid);
      this.grid.geometry.dispose();
      (this.grid.material as THREE.Material).dispose();
    }
    const grid = new THREE.GridHelper(
      200,
      20,
      dark ? 0x3a3f4b : 0xc8c8c8,
      dark ? 0x2a2e37 : 0xe0e0e0,
    );
    grid.rotation.x = Math.PI / 2; // grid in XY plane
    this.grid = grid;
    this.scene.add(grid);
  }

  /** Remove and dispose the current mesh, its geometry, and its material(s). */
  private clearMesh() {
    if (this.mesh) {
      this.scene.remove(this.mesh);
      this.geometry?.dispose();
      for (const m of this.materials) m.dispose();
      this.materials = [];
      this.mesh = null;
      this.geometry = null;
    }
  }

  private buildGeom(positions: Float32Array, normals: Float32Array): THREE.BufferGeometry {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geom.setAttribute("normal", new THREE.BufferAttribute(normals, 3));
    geom.computeBoundingBox();
    return geom;
  }

  /** Add the mesh to the scene with a black edge overlay, report its size, and
   *  frame it on first display. `material` may be a single material or a
   *  per-group array (for colored geometry). */
  private mountMesh(geom: THREE.BufferGeometry, material: THREE.Material | THREE.Material[]) {
    const mesh = new THREE.Mesh(geom, material);
    const edges = new THREE.LineSegments(
      new THREE.EdgesGeometry(geom, 20),
      new THREE.LineBasicMaterial({ color: 0x000000, opacity: 0.15, transparent: true }),
    );
    mesh.add(edges);

    this.mesh = mesh;
    this.geometry = geom;
    this.scene.add(mesh);

    const size = new THREE.Vector3();
    geom.boundingBox!.getSize(size);
    this.onInfo?.({ x: size.x, y: size.y, z: size.z });

    if (!this.hasFramed) {
      this.frame(geom);
      this.hasFramed = true;
    }
  }

  setMesh(positions: Float32Array, normals: Float32Array) {
    this.clearMesh();
    if (positions.length === 0) {
      this.onInfo?.(null);
      return;
    }
    const geom = this.buildGeom(positions, normals);
    const material = new THREE.MeshStandardMaterial({
      color: 0xf5a623,
      metalness: 0.1,
      roughness: 0.6,
      flatShading: true,
      side: THREE.DoubleSide,
    });
    this.materials = [material];
    this.mountMesh(geom, material);
  }

  /** Render `color()`/`#`/`%` groups: one geometry, one material per group via
   *  geometry groups. `#` highlight → translucent red, `%` background →
   *  translucent gray, solid → the group's color (transparent when alpha < 1). */
  setColoredMesh(positions: Float32Array, normals: Float32Array, groups: PreviewGroup[]) {
    this.clearMesh();
    if (positions.length === 0 || groups.length === 0) {
      this.setMesh(positions, normals);
      return;
    }
    const geom = this.buildGeom(positions, normals);
    const materials: THREE.Material[] = [];
    groups.forEach((g, i) => {
      geom.addGroup(g.start, g.count, i);
      materials.push(materialForGroup(g));
    });
    this.materials = materials;
    this.mountMesh(geom, materials);
  }

  /** Register the per-statement provenance soup for this render. Builds the
   *  (off-scene) pick mesh used for model→code selection and code→model
   *  highlighting, and clears any stale highlight. Pass empty data to disable
   *  picking (e.g. a model with no geometry, which has no provenance channel). */
  setProvenance(positions: Float32Array, normals: Float32Array, groups: ProvenanceGroup[]) {
    this.highlightSpan(null);
    this.pickGeometry?.dispose();
    this.pickGeometry = null;
    this.pickMesh = null;
    this.provPositions = positions;
    this.provNormals = normals;
    this.provGroups = groups;
    if (positions.length === 0 || groups.length === 0) return;
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geom.setAttribute("normal", new THREE.BufferAttribute(normals, 3));
    this.pickGeometry = geom;
    // Not added to the scene (invisible); raycasting doesn't require scene
    // membership, only an up-to-date world matrix. The soup is already in world
    // coordinates, so the identity transform is correct.
    this.pickMesh = new THREE.Mesh(geom, PICK_MATERIAL);
    this.pickMesh.updateMatrixWorld(true);
  }

  /** Register a model→code pick callback (fires with the picked statement's span,
   *  or `null` when the click hit empty space). Returns an unsubscribe fn. */
  onPick(cb: (span: Span | null) => void): () => void {
    this.onPickCb = cb;
    return () => {
      if (this.onPickCb === cb) this.onPickCb = null;
    };
  }

  /** Raycast the provenance pick mesh at a screen point and report the enclosing
   *  statement's span to the pick callback. */
  private pickAt(clientX: number, clientY: number) {
    if (!this.pickMesh || !this.onPickCb) return;
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.pointer.x = ((clientX - rect.left) / rect.width) * 2 - 1;
    this.pointer.y = -((clientY - rect.top) / rect.height) * 2 + 1;
    this.raycaster.setFromCamera(this.pointer, this.camera);
    const hit = this.raycaster.intersectObject(this.pickMesh, false)[0];
    if (hit?.faceIndex == null) return;
    const g = this.groupForFace(hit.faceIndex);
    this.onPickCb(g?.span ?? null);
  }

  /** The provenance group owning triangle `faceIndex` (soup vertex ranges are in
   *  multiples of 3, so a triangle t spans verts [3t, 3t+3)). */
  private groupForFace(faceIndex: number): ProvenanceGroup | undefined {
    return this.provGroups.find(
      (g) => faceIndex >= g.start / 3 && faceIndex < (g.start + g.count) / 3,
    );
  }

  /** Highlight the geometry produced by the statement at `span` (an overlay wash
   *  drawn on top), or clear the highlight when `span` is `null`. All groups
   *  sharing the span are highlighted (e.g. a `for` loop's instances). */
  highlightSpan(span: Span | null) {
    if (this.highlightMesh) {
      this.scene.remove(this.highlightMesh);
      this.highlightMesh.geometry.dispose();
      this.highlightMesh = null;
    }
    if (!span || this.provPositions.length === 0) return;
    // Collect the vertex ranges of every group with this exact span.
    const ranges = this.provGroups.filter(
      (g) => g.span && g.span[0] === span[0] && g.span[1] === span[1],
    );
    if (ranges.length === 0) return;
    let total = 0;
    for (const g of ranges) total += g.count * 3;
    const pos = new Float32Array(total);
    const nrm = new Float32Array(total);
    let off = 0;
    for (const g of ranges) {
      const s = g.start * 3;
      const len = g.count * 3;
      pos.set(this.provPositions.subarray(s, s + len), off);
      nrm.set(this.provNormals.subarray(s, s + len), off);
      off += len;
    }
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.BufferAttribute(pos, 3));
    geom.setAttribute("normal", new THREE.BufferAttribute(nrm, 3));
    const mesh = new THREE.Mesh(geom, HIGHLIGHT_MATERIAL);
    mesh.renderOrder = 999; // draw last, on top (material has depthTest off)
    this.highlightMesh = mesh;
    this.scene.add(mesh);
  }

  /** Unit view direction (camera → target points opposite this) + up vector. */
  private presetVectors(p: ViewPreset): { dir: THREE.Vector3; up: THREE.Vector3 } {
    const Z = new THREE.Vector3(0, 0, 1);
    const Y = new THREE.Vector3(0, 1, 0);
    switch (p) {
      case "front":
        return { dir: new THREE.Vector3(0, -1, 0), up: Z };
      case "back":
        return { dir: new THREE.Vector3(0, 1, 0), up: Z };
      case "right":
        return { dir: new THREE.Vector3(1, 0, 0), up: Z };
      case "left":
        return { dir: new THREE.Vector3(-1, 0, 0), up: Z };
      case "top":
        return { dir: new THREE.Vector3(0, 0, 1), up: Y };
      case "bottom":
        return { dir: new THREE.Vector3(0, 0, -1), up: Y };
      default:
        return { dir: new THREE.Vector3(0.6, -0.8, 0.5), up: Z };
    }
  }

  /** Size the ortho frustum to a given half-height, deriving width from the
   *  canvas aspect. */
  private setOrthoFrustum(halfHeight: number) {
    const canvas = this.renderer.domElement;
    const aspect = canvas.clientWidth / canvas.clientHeight || 1;
    this.orthoCamera.top = halfHeight;
    this.orthoCamera.bottom = -halfHeight;
    this.orthoCamera.left = -halfHeight * aspect;
    this.orthoCamera.right = halfHeight * aspect;
    this.orthoCamera.zoom = 1;
    this.orthoCamera.updateProjectionMatrix();
  }

  private frame(geom: THREE.BufferGeometry) {
    const box = geom.boundingBox!;
    const size = new THREE.Vector3();
    const center = new THREE.Vector3();
    box.getSize(size);
    box.getCenter(center);
    const radius = Math.max(size.x, size.y, size.z) * 0.75 + 1;
    // Position both cameras at the same distance (from the perspective fov) so
    // toggling projection after a frame keeps the eye in place.
    const dist = radius / Math.sin((this.perspCamera.fov * Math.PI) / 360);
    const { dir, up } = this.presetVectors(this.preset);
    this.camera.up.copy(up);
    this.camera.position.copy(center.clone().add(dir.clone().normalize().multiplyScalar(dist)));
    if (this.camera instanceof THREE.OrthographicCamera) this.setOrthoFrustum(radius);
    this.controls.target.copy(center);
    this.controls.update();
  }

  /** Switch between perspective and orthographic projection, preserving the eye
   *  position, orbit target, and apparent size of the model. */
  setProjection(mode: Projection) {
    if (mode === this.projection) return;
    const from = this.camera;
    const target = this.controls.target.clone();
    const to = mode === "orthographic" ? this.orthoCamera : this.perspCamera;

    to.position.copy(from.position);
    to.up.copy(from.up);
    to.quaternion.copy(from.quaternion);

    const dir = from.position.clone().sub(target);
    const dist = dir.length();
    const halfFov = (this.perspCamera.fov * Math.PI) / 360;
    if (mode === "orthographic") {
      // Match the perspective frustum height at the target plane.
      this.setOrthoFrustum(dist * Math.tan(halfFov));
    } else {
      // Move the eye so the perspective frustum spans the ortho view's height.
      const halfH = ((this.orthoCamera.top - this.orthoCamera.bottom) / 2) / this.orthoCamera.zoom;
      const d = halfH / Math.tan(halfFov);
      to.position.copy(target.clone().add(dir.normalize().multiplyScalar(d)));
      to.updateProjectionMatrix();
    }

    this.camera = to;
    this.projection = mode;

    // OrbitControls is bound to a single camera; rebind to the new one.
    this.controls.dispose();
    this.controls = this.makeControls();
    this.controls.target.copy(target);
    this.controls.update();
  }

  getProjection(): Projection {
    return this.projection;
  }

  /** Snap the camera to a named orientation, keeping the model framed. */
  setPreset(p: ViewPreset) {
    this.preset = p;
    if (this.geometry) this.frame(this.geometry);
  }

  resetView() {
    this.preset = "iso";
    if (this.geometry) this.frame(this.geometry);
  }

  /** The current camera as OpenSCAD `$vp*` values. `vpt`/`vpd`/`vpf` are exact;
   *  `vpr` is a best-effort Euler (roll = 0) matching the gimbal convention used
   *  by `setCamera` and the CLI rasterizer. */
  getCamera(): { vpr: [number, number, number]; vpt: [number, number, number]; vpd: number; vpf: number } {
    const t = this.controls.target;
    const dir = this.camera.position.clone().sub(t).normalize(); // target → eye
    const vpd = this.camera.position.distanceTo(t);
    const deg = (r: number) => (r * 180) / Math.PI;
    const clamp = (x: number) => Math.max(-1, Math.min(1, x));
    const rx = Math.asin(clamp(-dir.y));
    const ry = Math.atan2(dir.x, dir.z);
    return {
      vpr: [deg(rx), deg(ry), 0],
      vpt: [t.x, t.y, t.z],
      vpd,
      vpf: this.perspCamera.fov,
    };
  }

  /** Move the camera to OpenSCAD `$vp*` values (gimbal: `eye = target + dist ·
   *  Rz·Ry·Rx · +Z`), matching the CLI rasterizer's convention. */
  setCamera(v: {
    vpr?: [number, number, number] | null;
    vpt?: [number, number, number] | null;
    vpd?: number | null;
    vpf?: number | null;
  }) {
    const cur = this.getCamera();
    const vpr = v.vpr ?? cur.vpr;
    const vpt = v.vpt ?? cur.vpt;
    const vpd = v.vpd ?? cur.vpd;
    const vpf = v.vpf ?? cur.vpf;
    const rad = (d: number) => (d * Math.PI) / 180;
    const X = new THREE.Vector3(1, 0, 0);
    const Y = new THREE.Vector3(0, 1, 0);
    const Z = new THREE.Vector3(0, 0, 1);
    const rot = (base: THREE.Vector3) =>
      base
        .clone()
        .applyAxisAngle(X, rad(vpr[0]))
        .applyAxisAngle(Y, rad(vpr[1]))
        .applyAxisAngle(Z, rad(vpr[2]));
    const target = new THREE.Vector3(vpt[0], vpt[1], vpt[2]);
    const eye = target.clone().add(rot(new THREE.Vector3(0, 0, 1)).multiplyScalar(vpd));
    this.camera.position.copy(eye);
    this.camera.up.copy(rot(new THREE.Vector3(0, 1, 0)).normalize());
    this.controls.target.copy(target);
    if (this.perspCamera.fov !== vpf) {
      this.perspCamera.fov = vpf;
      this.perspCamera.updateProjectionMatrix();
    }
    this.controls.update();
  }

  /** Register a camera-change callback (OrbitControls `change`). Returns an
   *  unsubscribe fn. */
  onCameraChange(cb: () => void): () => void {
    this.changeListeners.add(cb);
    this.controls.addEventListener("change", cb);
    return () => {
      this.changeListeners.delete(cb);
      this.controls.removeEventListener("change", cb);
    };
  }

  /** Capture the current view as a PNG blob (renders one frame first). */
  capturePng(): Promise<Blob> {
    this.renderer.render(this.scene, this.camera);
    return new Promise((resolve, reject) => {
      this.renderer.domElement.toBlob((blob) => {
        if (blob) resolve(blob);
        else reject(new Error("canvas capture failed"));
      }, "image/png");
    });
  }
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

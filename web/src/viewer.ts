// A minimal three.js orbit viewer with a grid, axes, and flat-shaded mesh.
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import { viewerConst, viewerTheme, hex, type ViewerTheme } from "./tokens";

/** Model bounding-box size (mm) reported after each render. */
export interface MeshInfo {
  x: number;
  y: number;
  z: number;
}

/** A named camera orientation. */
export type ViewPreset =
  "iso" | "front" | "back" | "top" | "bottom" | "right" | "left";

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
 *  with the stack of enclosing source spans that produced it (outermost first,
 *  innermost last; empty when unattributable). A click selects the deepest (last)
 *  span; the cursor→model highlight matches any span in the stack by containment. */
export interface ProvenanceGroup {
  start: number;
  count: number;
  spans: Span[];
}

/** Overlay material for the code→model highlight: a bright wash drawn on top of
 *  the model (depthTest off) so the selected geometry reads clearly regardless of
 *  the model's own per-group colors. */
const HIGHLIGHT_MATERIAL = new THREE.MeshBasicMaterial({
  color: viewerConst.selection,
  transparent: true,
  opacity: 0.225,
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
  /** The adaptive floor grid together with the world axes and their numeric
   *  tick labels, held as one group so it can be swapped out wholesale. Rebuilt
   *  whenever the zoom-derived spacing, the target-snapped center, or the theme
   *  changes. */
  private gridGroup: THREE.Group | null = null;
  private themeDark = false;
  /** Current viewer palette, updated on every setTheme. */
  private vt: ViewerTheme = viewerTheme("dark");
  /** Cache key of the last-built grid, so per-frame `updateGrid` calls that
   *  wouldn't change anything early-return instead of rebuilding. */
  private lastGridKey = "";

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

  // ---- navigation view cube (top-right gizmo) ----
  private cubeRenderer: THREE.WebGLRenderer | null = null;
  private cubeScene: THREE.Scene | null = null;
  private cubeCamera: THREE.PerspectiveCamera | null = null;
  private cube: THREE.Mesh | null = null;
  /** Overlay box that highlights the hovered region (face / edge / corner). */
  private cubeHighlight: THREE.Mesh | null = null;
  private cubeRay = new THREE.Raycaster();
  private cubePointer = new THREE.Vector2();
  /** In-flight pointer drag on the cube: last position + whether it became a drag. */
  private cubeDrag: { x: number; y: number; moved: boolean } | null = null;
  /** Sign-pattern key of the highlighted region (`"sx,sy,sz"`, `""` when none). */
  private cubeHover = "";
  /** Handle for the render loop + the window resize handler, so `dispose` can
   *  stop the loop and detach the listener (otherwise a torn-down Viewer keeps
   *  animating — e.g. React StrictMode's throwaway first mount, whose leftover
   *  view cube would ghost behind the live one). */
  private rafId = 0;
  private onWindowResize = () => this.resize();
  /** Active camera fly-to (face click), interpolated in the animate loop. */
  private camAnim: {
    fromDir: THREE.Vector3;
    toDir: THREE.Vector3;
    fromUp: THREE.Vector3;
    toUp: THREE.Vector3;
    dist: number;
    t0: number;
    dur: number;
  } | null = null;

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

    // Background + grid colors follow the OS appearance; setTheme builds the
    // initial adaptive grid (with axes + labels) so the first frame matches.
    this.setTheme(
      window.matchMedia("(prefers-color-scheme: dark)").matches
        ? "dark"
        : "light",
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

    this.setupViewCube(canvas.parentElement);

    this.resize();
    window.addEventListener("resize", this.onWindowResize);
    this.animate();
  }

  /** Bind fresh OrbitControls to the active camera, re-attaching any registered
   *  change listeners. Called on construction and whenever the projection (and
   *  thus the camera instance) changes. */
  private makeControls(): OrbitControls {
    const controls = new OrbitControls(this.camera, this.renderer.domElement);
    controls.enableDamping = true;
    // Re-derive the grid spacing/extent from the new camera state whenever the
    // view moves (throttled by updateGrid's cache key).
    controls.addEventListener("change", () => this.updateGrid());
    for (const cb of this.changeListeners)
      controls.addEventListener("change", cb);
    return controls;
  }

  private animate = () => {
    this.rafId = requestAnimationFrame(this.animate);
    this.stepCamAnim();
    this.controls.update();
    this.renderer.render(this.scene, this.camera);
    this.renderViewCube();
  };

  /** Tear down: stop the render loop, detach listeners, and remove the view-cube
   *  overlay. Idempotent. Call when the owning component unmounts. */
  dispose() {
    cancelAnimationFrame(this.rafId);
    window.removeEventListener("resize", this.onWindowResize);
    this.controls.dispose();
    if (this.cubeRenderer) {
      this.cubeRenderer.dispose();
      this.cubeRenderer.domElement.remove();
      this.cubeRenderer = null;
    }
  }

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
    this.updateGrid();
  }

  /** Apply the OS light/dark appearance to the scene: background color and the
   *  floor grid. Forces a grid rebuild so the new theme colors take effect; safe
   *  to call repeatedly on flips. */
  setTheme(mode: ThemeMode) {
    this.themeDark = mode === "dark";
    this.vt = viewerTheme(mode);
    this.scene.background = new THREE.Color(this.vt.background);
    this.updateGrid(true);
    this.rebuildCubeFaces();
  }

  /** The world-space vertical extent currently visible at the orbit-target plane
   *  — the basis for picking a power-of-ten grid spacing. */
  private visibleWorldHeight(): number {
    if (this.camera instanceof THREE.OrthographicCamera) {
      return (this.camera.top - this.camera.bottom) / this.camera.zoom;
    }
    const dist = this.camera.position.distanceTo(this.controls.target);
    return 2 * dist * Math.tan((this.perspCamera.fov * Math.PI) / 360);
  }

  /** Recompute the zoom-adaptive grid and rebuild it when its spacing, extent, or
   *  target-snapped center has changed (or `force`, e.g. on a theme flip). Cheap
   *  to call every frame: unchanged views hit the cache-key early return. */
  private updateGrid(force = false) {
    const visH = Math.max(this.visibleWorldHeight(), 1e-4);
    const el = this.renderer.domElement;
    const aspect =
      el.clientWidth && el.clientHeight ? el.clientWidth / el.clientHeight : 1;
    const span = Math.max(visH, visH * aspect);
    // Nearest power of ten that puts ~10 major cells across the view: 1000mm
    // visible → 100mm cells, ~100mm → 10mm, ~10mm → 1mm, and so on.
    const spacing = Math.pow(10, Math.round(Math.log10(span / 10)));
    // ~10 grid lines across the view (halfCells each way from center), so the
    // grid stays sparse; labels then land on every other line (see buildGrid).
    const halfCells = Math.min(40, Math.max(5, Math.ceil(span / spacing / 2)));
    // Center on the orbit target, snapped to the grid so it slides cell-by-cell
    // and always fills the view as you pan.
    const cx = Math.round(this.controls.target.x / spacing);
    const cy = Math.round(this.controls.target.y / spacing);
    // Looking straight down an axis collapses that axis's tick labels onto the
    // origin; drop them (only near exact top/front/side views, so iso keeps all).
    const dir = this.camera.position
      .clone()
      .sub(this.controls.target)
      .normalize();
    const suppress = {
      x: Math.abs(dir.x) > 0.95,
      y: Math.abs(dir.y) > 0.95,
      z: Math.abs(dir.z) > 0.95,
    };
    const sKey = `${+suppress.x}${+suppress.y}${+suppress.z}`;
    const key = `${spacing}:${halfCells}:${cx}:${cy}:${this.themeDark}:${sKey}`;
    if (!force && key === this.lastGridKey) return;
    this.lastGridKey = key;
    this.buildGrid(spacing, halfCells, cx * spacing, cy * spacing, suppress);
  }

  /** Remove and dispose the current grid group (lines, axes, label sprites and
   *  their canvas textures). */
  private disposeGrid() {
    if (!this.gridGroup) return;
    this.scene.remove(this.gridGroup);
    this.gridGroup.traverse((o) => {
      const obj = o as THREE.Mesh & { material?: THREE.Material };
      obj.geometry?.dispose();
      const mat = obj.material as
        (THREE.Material & { map?: THREE.Texture }) | undefined;
      if (mat) {
        mat.map?.dispose();
        mat.dispose();
      }
    });
    this.gridGroup = null;
  }

  /** Build the floor grid centered at (`ox`,`oy`) with `spacing`-mm cells,
   *  `halfCells` out from the center each way, plus the X/Y/Z world axes and the
   *  numeric tick labels running along them. The grid follows the orbit target so
   *  it always fills the view; the labeled axes stay anchored at the origin. */
  private buildGrid(
    spacing: number,
    halfCells: number,
    ox: number,
    oy: number,
    suppress: { x: boolean; y: boolean; z: boolean } = {
      x: false,
      y: false,
      z: false,
    },
  ) {
    this.disposeGrid();
    const g = new THREE.Group();
    const half = halfCells * spacing;
    const loX = ox - half,
      hiX = ox + half,
      loY = oy - half,
      hiY = oy + half;

    // --- grid lines ---
    const pts: number[] = [];
    for (let i = -halfCells; i <= halfCells; i++) {
      const x = ox + i * spacing;
      pts.push(x, loY, 0, x, hiY, 0);
      const y = oy + i * spacing;
      pts.push(loX, y, 0, hiX, y, 0);
    }
    const gridGeom = new THREE.BufferGeometry();
    gridGeom.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
    g.add(
      new THREE.LineSegments(
        gridGeom,
        new THREE.LineBasicMaterial({
          color: this.vt.gridLine,
          transparent: true,
          opacity: this.vt.gridLineOpacity,
        }),
      ),
    );

    // --- number formatting + label helper ---
    const dec = spacing < 1 ? Math.max(0, Math.round(-Math.log10(spacing))) : 0;
    const zero = (0).toFixed(dec);
    const fmt = (v: number) => {
      const s = v.toFixed(dec);
      return s === `-${zero}` ? zero : s;
    };
    const lh = spacing * 0.24; // label height in mm → ~constant on screen
    const numCol = this.vt.gridLabel;
    const addLabel = (
      text: string,
      color: string,
      x: number,
      y: number,
      z: number,
      scale = 1,
    ) => {
      const s = this.makeTickLabel(text, color, lh * scale);
      s.position.set(x, y, z);
      g.add(s);
    };

    // --- world axes + tick labels (only where the axis crosses the grid) ---
    const X_COL = viewerConst.axisX,
      Y_COL = viewerConst.axisY,
      Z_COL = viewerConst.axisZ;
    const line = (a: number[], b: number[], color: number) => {
      const geo = new THREE.BufferGeometry();
      geo.setAttribute(
        "position",
        new THREE.Float32BufferAttribute([...a, ...b], 3),
      );
      g.add(
        new THREE.LineSegments(geo, new THREE.LineBasicMaterial({ color })),
      );
    };
    // Ruler ticks perpendicular to an axis: a longer (major) mark at every
    // labeled line and a shorter (minor) mark halfway to the next label.
    const majorH = spacing * 0.28,
      minorH = spacing * 0.15;
    const ticks = (
      axis: "x" | "y" | "z",
      lo: number,
      hi: number,
      color: number,
    ) => {
      const pts: number[] = [];
      for (
        let v = Math.ceil(lo / spacing) * spacing;
        v <= hi + 1e-6;
        v += spacing
      ) {
        const h = Math.round(v / spacing) % 2 === 0 ? majorH : minorH;
        if (axis === "x") pts.push(v, -h, 0, v, h, 0);
        else if (axis === "y") pts.push(-h, v, 0, h, v, 0);
        else pts.push(-h, 0, v, h, 0, v);
      }
      if (pts.length) {
        const geo = new THREE.BufferGeometry();
        geo.setAttribute("position", new THREE.Float32BufferAttribute(pts, 3));
        g.add(
          new THREE.LineSegments(geo, new THREE.LineBasicMaterial({ color })),
        );
      }
    };
    const xAxisOnGrid = loY <= 0 && 0 <= hiY; // the X axis (y=0) is within the grid
    const yAxisOnGrid = loX <= 0 && 0 <= hiX; // the Y axis (x=0) is within the grid
    const originOnGrid = xAxisOnGrid && yAxisOnGrid;

    // Label only every other grid line (even multiples of the spacing), so
    // there are ~half as many labels as lines, symmetrically about the origin.
    const labeled = (coord: number) => Math.round(coord / spacing) % 2 === 0;

    if (xAxisOnGrid) {
      line([loX, 0, 0], [hiX, 0, 0], X_COL);
      addLabel("X", hex(X_COL), hiX + lh * 1.8, -lh * 1.4, 0, 1.5);
      if (!suppress.x) {
        ticks("x", loX, hiX, X_COL);
        for (
          let x = Math.ceil(loX / spacing) * spacing;
          x <= hiX + 1e-6;
          x += spacing
        ) {
          if (Math.abs(x) < spacing / 2 || !labeled(x)) continue; // origin labeled below
          addLabel(fmt(x), numCol, x, -lh * 1.6, 0);
        }
      }
    }
    if (yAxisOnGrid) {
      line([0, loY, 0], [0, hiY, 0], Y_COL);
      addLabel("Y", hex(Y_COL), -lh * 1.4, hiY + lh * 1.8, 0, 1.5);
      if (!suppress.y) {
        ticks("y", loY, hiY, Y_COL);
        for (
          let y = Math.ceil(loY / spacing) * spacing;
          y <= hiY + 1e-6;
          y += spacing
        ) {
          if (Math.abs(y) < spacing / 2 || !labeled(y)) continue;
          addLabel(fmt(y), numCol, -lh * 1.6, y, 0);
        }
      }
    }
    if (originOnGrid) {
      // Vertical Z axis rising from the origin, with its own tick labels.
      line([0, 0, 0], [0, 0, half], Z_COL);
      addLabel("Z", hex(Z_COL), -lh * 1.4, -lh * 1.4, half + lh * 1.8, 1.5);
      if (!suppress.z) {
        ticks("z", 0, half, Z_COL);
        for (let i = 2; i <= halfCells; i += 2) {
          addLabel(
            fmt(i * spacing),
            this.vt.axisTick,
            -lh * 1.6,
            -lh * 1.6,
            i * spacing,
          );
        }
        // Origin label, shown only when the flat X/Y ticks aren't stacking on it.
        if (!suppress.x && !suppress.y)
          addLabel("0", numCol, -lh * 1.6, -lh * 1.6, 0);
      }
    }

    this.gridGroup = g;
    this.scene.add(g);
  }

  /** A camera-facing text sprite `worldHeight` mm tall. Because the grid spacing
   *  (and thus label height) scales with the zoom level, labels stay roughly
   *  constant in screen size across zooms. */
  private makeTickLabel(
    text: string,
    color: string,
    worldHeight: number,
  ): THREE.Sprite {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const fontPx = 64;
    const font = `bold ${fontPx}px sans-serif`;
    const canvas = document.createElement("canvas");
    const ctx = canvas.getContext("2d")!;
    ctx.font = font;
    const w = Math.ceil(ctx.measureText(text).width) + 8;
    const h = fontPx + 8;
    canvas.width = Math.ceil(w * dpr);
    canvas.height = Math.ceil(h * dpr);
    ctx.scale(dpr, dpr);
    ctx.font = font;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillStyle = color;
    ctx.fillText(text, w / 2, h / 2);
    const tex = new THREE.CanvasTexture(canvas);
    tex.minFilter = THREE.LinearFilter;
    tex.magFilter = THREE.LinearFilter;
    const sprite = new THREE.Sprite(
      new THREE.SpriteMaterial({
        map: tex,
        transparent: true,
        depthWrite: false,
      }),
    );
    sprite.scale.set(worldHeight * (w / h), worldHeight, 1);
    sprite.renderOrder = 10;
    return sprite;
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

  private buildGeom(
    positions: Float32Array,
    normals: Float32Array,
  ): THREE.BufferGeometry {
    const geom = new THREE.BufferGeometry();
    geom.setAttribute("position", new THREE.BufferAttribute(positions, 3));
    geom.setAttribute("normal", new THREE.BufferAttribute(normals, 3));
    geom.computeBoundingBox();
    return geom;
  }

  /** Add the mesh to the scene with a black edge overlay, report its size, and
   *  frame it on first display. `material` may be a single material or a
   *  per-group array (for colored geometry). */
  private mountMesh(
    geom: THREE.BufferGeometry,
    material: THREE.Material | THREE.Material[],
  ) {
    const mesh = new THREE.Mesh(geom, material);
    const edges = new THREE.LineSegments(
      new THREE.EdgesGeometry(geom, 20),
      new THREE.LineBasicMaterial({
        color: viewerConst.edge,
        opacity: 0.15,
        transparent: true,
      }),
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
      color: viewerConst.mesh,
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
  setColoredMesh(
    positions: Float32Array,
    normals: Float32Array,
    groups: PreviewGroup[],
  ) {
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
  setProvenance(
    positions: Float32Array,
    normals: Float32Array,
    groups: ProvenanceGroup[],
  ) {
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
    if (hit?.faceIndex == null) {
      // Clicked empty space: report a null pick so the app can deselect.
      this.onPickCb(null);
      return;
    }
    const g = this.groupForFace(hit.faceIndex);
    // Select the deepest (innermost) statement span for the clicked geometry.
    const deepest = g && g.spans.length ? g.spans[g.spans.length - 1] : null;
    this.onPickCb(deepest);
  }

  /** The provenance group owning triangle `faceIndex` (soup vertex ranges are in
   *  multiples of 3, so a triangle t spans verts [3t, 3t+3)). */
  private groupForFace(faceIndex: number): ProvenanceGroup | undefined {
    return this.provGroups.find(
      (g) => faceIndex >= g.start / 3 && faceIndex < (g.start + g.count) / 3,
    );
  }

  /** Highlight the geometry produced by the statement at `span` (an overlay wash
   *  drawn on top), or clear the highlight when `span` is `null`. Every group
   *  whose span stack *contains* `span` is highlighted, so a span at any nesting
   *  level lights its whole subtree (e.g. a module call lights all its geometry,
   *  a `for` loop's instances, or every instance of a reused helper module). */
  highlightSpan(span: Span | null) {
    if (this.highlightMesh) {
      this.scene.remove(this.highlightMesh);
      this.highlightMesh.geometry.dispose();
      this.highlightMesh = null;
    }
    if (!span || this.provPositions.length === 0) return;
    // Collect the vertex ranges of every group whose stack contains this span.
    const ranges = this.provGroups.filter((g) =>
      g.spans.some((s) => s[0] === span[0] && s[1] === span[1]),
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
  private presetVectors(p: ViewPreset): {
    dir: THREE.Vector3;
    up: THREE.Vector3;
  } {
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
    this.camera.position.copy(
      center.clone().add(dir.clone().normalize().multiplyScalar(dist)),
    );
    if (this.camera instanceof THREE.OrthographicCamera)
      this.setOrthoFrustum(radius);
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
      const halfH =
        (this.orthoCamera.top - this.orthoCamera.bottom) /
        2 /
        this.orthoCamera.zoom;
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

  // ---------------------------------------------------------------------------
  // Navigation view cube
  // ---------------------------------------------------------------------------

  /** Material order of a `BoxGeometry`'s faces → the world direction each shows. */
  private static readonly CUBE_FACES: { label: string; preset: ViewPreset }[] =
    [
      { label: "RIGHT", preset: "right" }, // +X
      { label: "LEFT", preset: "left" }, //  -X
      { label: "BACK", preset: "back" }, //  +Y
      { label: "FRONT", preset: "front" }, // -Y
      { label: "TOP", preset: "top" }, //   +Z
      { label: "BOTTOM", preset: "bottom" }, // -Z
    ];

  /** Build the top-right navigation cube: a small transparent WebGL overlay whose
   *  orientation tracks the main camera. Drag it to orbit; click a face to fly to
   *  that view. No-op if there's no parent element to anchor the overlay to. */
  private setupViewCube(parent: HTMLElement | null) {
    if (!parent) return;
    const SIZE = 92;
    const canvas = document.createElement("canvas");
    Object.assign(canvas.style, {
      position: "absolute",
      top: "10px",
      right: "10px",
      width: `${SIZE}px`,
      height: `${SIZE}px`,
      cursor: "grab",
      zIndex: "5",
      touchAction: "none",
    } as CSSStyleDeclaration);
    parent.appendChild(canvas);

    const renderer = new THREE.WebGLRenderer({
      canvas,
      antialias: true,
      alpha: true,
    });
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setSize(SIZE, SIZE, false);
    renderer.setClearColor(0x000000, 0);

    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(38, 1, 0.1, 100);
    camera.up.set(0, 0, 1);

    const cube = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), []);
    const edges = new THREE.LineSegments(
      new THREE.EdgesGeometry(cube.geometry),
      new THREE.LineBasicMaterial({
        color: viewerConst.edge,
        transparent: true,
        opacity: 0.25,
      }),
    );
    cube.add(edges);
    // Hover highlight: an accent box reshaped over the hovered face/edge/corner,
    // drawn on top (depthTest off) so it reads regardless of the cube's faces.
    const highlight = new THREE.Mesh(
      new THREE.BoxGeometry(1, 1, 1),
      new THREE.MeshBasicMaterial({
        color: viewerConst.selection,
        transparent: true,
        opacity: 0.5,
        depthTest: false,
        depthWrite: false,
      }),
    );
    highlight.renderOrder = 20;
    highlight.visible = false;
    cube.add(highlight);
    scene.add(cube);

    this.cubeRenderer = renderer;
    this.cubeScene = scene;
    this.cubeCamera = camera;
    this.cube = cube;
    this.cubeHighlight = highlight;
    this.rebuildCubeFaces();

    canvas.addEventListener("pointerdown", (e) => {
      if (e.button !== 0) return;
      this.camAnim = null; // a fresh interaction cancels any in-flight fly-to
      this.cubeDrag = { x: e.clientX, y: e.clientY, moved: false };
      this.setCubeHover(null); // hide the region highlight while dragging
      canvas.setPointerCapture(e.pointerId);
      canvas.style.cursor = "grabbing";
    });
    canvas.addEventListener("pointermove", (e) => {
      if (this.cubeDrag) {
        const dx = e.clientX - this.cubeDrag.x;
        const dy = e.clientY - this.cubeDrag.y;
        if (Math.hypot(dx, dy) > 3) this.cubeDrag.moved = true;
        if (this.cubeDrag.moved) {
          this.orbitBy(dx * 0.01, dy * 0.01);
          this.cubeDrag.x = e.clientX;
          this.cubeDrag.y = e.clientY;
        }
      } else {
        this.setCubeHover(this.cubePick(e.clientX, e.clientY)?.region ?? null);
      }
    });
    const end = (e: PointerEvent) => {
      const drag = this.cubeDrag;
      this.cubeDrag = null;
      canvas.style.cursor = "grab";
      try {
        canvas.releasePointerCapture(e.pointerId);
      } catch {
        /* pointer already released */
      }
      if (drag && !drag.moved) {
        const hit = this.cubePick(e.clientX, e.clientY);
        if (hit) this.animateToDir(hit.dir, hit.up);
      }
    };
    canvas.addEventListener("pointerup", end);
    canvas.addEventListener("pointercancel", end);
    canvas.addEventListener("pointerleave", () => this.setCubeHover(null));
  }

  /** (Re)build the six labeled face materials for the current theme. */
  private rebuildCubeFaces() {
    if (!this.cube) return;
    const old = this.cube.material as THREE.Material[];
    if (Array.isArray(old))
      for (const m of old) {
        (m as THREE.MeshBasicMaterial).map?.dispose();
        m.dispose();
      }
    this.cube.material = Viewer.CUBE_FACES.map(
      (f) =>
        new THREE.MeshBasicMaterial({
          map: this.makeCubeFaceTexture(f.label),
          transparent: true,
        }),
    );
  }

  /** A 128² canvas texture for one cube face: filled panel, border, centered label. */
  private makeCubeFaceTexture(label: string): THREE.CanvasTexture {
    const s = 128;
    const canvas = document.createElement("canvas");
    canvas.width = canvas.height = s;
    const ctx = canvas.getContext("2d")!;
    ctx.fillStyle = this.vt.cubeFace;
    ctx.fillRect(0, 0, s, s);
    ctx.strokeStyle = this.vt.cubeStroke;
    ctx.lineWidth = 6;
    ctx.strokeRect(3, 3, s - 6, s - 6);
    ctx.fillStyle = this.vt.cubeText;
    const fs = label.length > 5 ? 20 : 24;
    ctx.font = `bold ${fs}px sans-serif`;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(label, s / 2, s / 2 + 2);
    const tex = new THREE.CanvasTexture(canvas);
    tex.anisotropy = this.cubeRenderer?.capabilities.getMaxAnisotropy() ?? 1;
    return tex;
  }

  /** Raycast the cube → the hovered face index (for tinting) and the view
   *  direction to fly to. A hit near a face centre gives that face-on view; near
   *  an edge or corner it gives the edge / corner (isometric) view, so clicking a
   *  corner frames the model isometrically from that corner. */
  private cubePick(
    clientX: number,
    clientY: number,
  ): {
    faceIndex: number;
    region: THREE.Vector3;
    dir: THREE.Vector3;
    up: THREE.Vector3;
  } | null {
    if (!this.cube || !this.cubeCamera || !this.cubeRenderer) return null;
    const rect = this.cubeRenderer.domElement.getBoundingClientRect();
    this.cubePointer.x = ((clientX - rect.left) / rect.width) * 2 - 1;
    this.cubePointer.y = -((clientY - rect.top) / rect.height) * 2 + 1;
    this.cubeRay.setFromCamera(this.cubePointer, this.cubeCamera);
    const hit = this.cubeRay.intersectObject(this.cube, false)[0];
    if (!hit || !hit.face) return null;
    // The cube is axis-aligned (only the camera moves), so the local face normal
    // is a world axis → the material index of the hovered face.
    const n = hit.face.normal;
    const ax =
      Math.abs(n.x) > Math.abs(n.y) && Math.abs(n.x) > Math.abs(n.z)
        ? 0
        : Math.abs(n.y) > Math.abs(n.z)
          ? 1
          : 2;
    const faceIndex =
      ax === 0
        ? n.x > 0
          ? 0
          : 1
        : ax === 1
          ? n.y > 0
            ? 2
            : 3
          : n.z > 0
            ? 4
            : 5;
    // hit.point is world == local (cube at the origin, unit size), so each
    // component is in [-0.5, 0.5]. Components near an extreme (|c| > T) become
    // ±1: one → a face, two → an edge, three → a corner (isometric) direction.
    const p = hit.point;
    const T = 0.3;
    // The clicked region as an integer sign pattern: one nonzero → a face, two →
    // an edge, three → a corner. Drives the hover highlight (`region`) directly.
    const region = new THREE.Vector3(
      Math.abs(p.x) > T ? Math.sign(p.x) : 0,
      Math.abs(p.y) > T ? Math.sign(p.y) : 0,
      Math.abs(p.z) > T ? Math.sign(p.z) : 0,
    );
    if (region.lengthSq() === 0) region.copy(n).round(); // safety: fall back to the face
    // Keep world Z-up for every fly-to so main-canvas dragging always orbits
    // azimuth about Z. Straight top/bottom is the Z-up pole (OrbitControls can't
    // orbit there — left/right drag would spin about Y), so nudge a hair off it.
    const dir = region.clone().normalize();
    if (region.x === 0 && region.y === 0)
      dir.set(0, -0.05, region.z).normalize();
    return { faceIndex, region, dir, up: new THREE.Vector3(0, 0, 1) };
  }

  /** Position the hover highlight over the `region` (an integer sign pattern that
   *  selects a face / edge / corner), or hide it when `region` is null. Matches
   *  the face/edge/corner zones used by `cubePick` (mid band |c|≤0.3, outer band). */
  private setCubeHover(region: THREE.Vector3 | null) {
    const hl = this.cubeHighlight;
    if (!hl) return;
    const key = region
      ? `${Math.sign(region.x)},${Math.sign(region.y)},${Math.sign(region.z)}`
      : "";
    if (key === this.cubeHover) return;
    this.cubeHover = key;
    if (!region) {
      hl.visible = false;
      return;
    }
    // Per axis: a pinned side (±1) → a thin slab hugging that face; a free axis
    // (0) → span the middle. The pinned bands poke slightly proud of the surface.
    const band = (s: number) =>
      s === 0 ? { c: 0, h: 0.3 } : { c: 0.41 * s, h: 0.11 };
    const bx = band(Math.sign(region.x)),
      by = band(Math.sign(region.y)),
      bz = band(Math.sign(region.z));
    hl.position.set(bx.c, by.c, bz.c);
    hl.scale.set(bx.h * 2, by.h * 2, bz.h * 2);
    hl.visible = true;
  }

  /** Orbit the main camera by (azimuth, polar) radians about the target, keeping
   *  the eye distance fixed (Z-up spherical, mirroring OrbitControls). */
  private orbitBy(dAz: number, dPolar: number) {
    const target = this.controls.target;
    const off = this.camera.position.clone().sub(target);
    const r = off.length();
    if (r === 0) return;
    const polar = THREE.MathUtils.clamp(
      Math.acos(off.z / r) - dPolar,
      1e-3,
      Math.PI - 1e-3,
    );
    const az = Math.atan2(off.y, off.x) - dAz;
    off.set(
      r * Math.sin(polar) * Math.cos(az),
      r * Math.sin(polar) * Math.sin(az),
      r * Math.cos(polar),
    );
    this.camera.position.copy(target).add(off);
    this.camera.up.set(0, 0, 1);
    this.controls.update();
  }

  /** Start a smooth fly-to a view `dir` (unit eye offset from the target) with a
   *  given up vector, preserving the current target and eye distance (interpolated
   *  each frame by `stepCamAnim`). */
  private animateToDir(dir: THREE.Vector3, up: THREE.Vector3) {
    const target = this.controls.target;
    this.camAnim = {
      fromDir: this.camera.position.clone().sub(target).normalize(),
      toDir: dir.clone().normalize(),
      fromUp: this.camera.up.clone().normalize(),
      toUp: up.clone().normalize(),
      dist: this.camera.position.distanceTo(target),
      t0: performance.now(),
      dur: 350,
    };
  }

  /** Advance an in-flight fly-to by one frame (slerp direction, lerp up). */
  private stepCamAnim() {
    const a = this.camAnim;
    if (!a) return;
    const k = Math.min(1, (performance.now() - a.t0) / a.dur);
    const e = k < 0.5 ? 2 * k * k : 1 - Math.pow(-2 * k + 2, 2) / 2; // easeInOutQuad
    const dir = slerpUnit(a.fromDir, a.toDir, e);
    const target = this.controls.target;
    this.camera.position.copy(target).add(dir.multiplyScalar(a.dist));
    this.camera.up.copy(a.fromUp.clone().lerp(a.toUp, e).normalize());
    if (k >= 1) this.camAnim = null;
  }

  /** Sync the cube to the main camera's orientation and render it. */
  private renderViewCube() {
    if (!this.cubeRenderer || !this.cubeCamera || !this.cubeScene) return;
    const dir = this.camera.position.clone().sub(this.controls.target);
    if (dir.lengthSq() === 0) return;
    this.cubeCamera.position.copy(dir.normalize().multiplyScalar(3.4));
    this.cubeCamera.up.copy(this.camera.up);
    this.cubeCamera.lookAt(0, 0, 0);
    this.cubeRenderer.render(this.cubeScene, this.cubeCamera);
  }

  /** The current camera as OpenSCAD `$vp*` values. `vpt`/`vpd`/`vpf` are exact;
   *  `vpr` is a best-effort Euler (roll = 0) matching the gimbal convention used
   *  by `setCamera` and the CLI rasterizer. */
  getCamera(): {
    vpr: [number, number, number];
    vpt: [number, number, number];
    vpd: number;
    vpf: number;
  } {
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
    const eye = target
      .clone()
      .add(rot(new THREE.Vector3(0, 0, 1)).multiplyScalar(vpd));
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

/** Spherical interpolation between two unit vectors (returns a unit vector). */
function slerpUnit(
  a: THREE.Vector3,
  b: THREE.Vector3,
  t: number,
): THREE.Vector3 {
  const dot = THREE.MathUtils.clamp(a.dot(b), -1, 1);
  const theta = Math.acos(dot) * t;
  const rel = b.clone().sub(a.clone().multiplyScalar(dot));
  if (rel.lengthSq() < 1e-10) return a.clone().lerp(b, t).normalize();
  rel.normalize();
  return a
    .clone()
    .multiplyScalar(Math.cos(theta))
    .add(rel.multiplyScalar(Math.sin(theta)));
}

/** The three.js material for a preview group: `#` → translucent red, `%` →
 *  translucent gray, solid → the group color (transparent when alpha < 1). */
function materialForGroup(g: PreviewGroup): THREE.Material {
  const base = { flatShading: true, side: THREE.DoubleSide as THREE.Side };
  if (g.mode === "highlight") {
    return new THREE.MeshStandardMaterial({
      ...base,
      color: viewerConst.modifierHighlight,
      transparent: true,
      opacity: 0.5,
    });
  }
  if (g.mode === "background") {
    return new THREE.MeshStandardMaterial({
      ...base,
      color: viewerConst.modifierBackground,
      transparent: true,
      opacity: 0.3,
    });
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

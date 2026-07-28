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

/** A colored preview group: a triangle range (vertex offsets) + color + mode. */
export interface PreviewGroup {
  start: number;
  count: number;
  color: [number, number, number, number];
  mode: "solid" | "highlight" | "background";
}

export class Viewer {
  private renderer: THREE.WebGLRenderer;
  private scene: THREE.Scene;
  private camera: THREE.PerspectiveCamera;
  private controls: OrbitControls;
  private mesh: THREE.Mesh | null = null;
  private geometry: THREE.BufferGeometry | null = null;
  private materials: THREE.Material[] = [];
  private hasFramed = false;
  private preset: ViewPreset = "iso";

  constructor(
    canvas: HTMLCanvasElement,
    private onInfo?: (info: MeshInfo | null) => void,
  ) {
    this.renderer = new THREE.WebGLRenderer({ canvas, antialias: true });
    this.renderer.setPixelRatio(window.devicePixelRatio);

    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x1a1d23);

    this.camera = new THREE.PerspectiveCamera(45, 1, 0.01, 100000);
    this.camera.up.set(0, 0, 1); // Z-up, like OpenSCAD
    this.camera.position.set(60, -80, 50);

    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;

    const key = new THREE.DirectionalLight(0xffffff, 2.2);
    key.position.set(40, -60, 80);
    this.scene.add(key);
    const fill = new THREE.DirectionalLight(0xffffff, 0.7);
    fill.position.set(-50, 40, 20);
    this.scene.add(fill);
    this.scene.add(new THREE.AmbientLight(0xffffff, 0.5));

    const grid = new THREE.GridHelper(200, 20, 0x3a3f4b, 0x2a2e37);
    grid.rotation.x = Math.PI / 2; // grid in XY plane
    this.scene.add(grid);
    this.scene.add(new THREE.AxesHelper(20));

    this.resize();
    window.addEventListener("resize", () => this.resize());
    this.animate();
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
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
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

  private frame(geom: THREE.BufferGeometry) {
    const box = geom.boundingBox!;
    const size = new THREE.Vector3();
    const center = new THREE.Vector3();
    box.getSize(size);
    box.getCenter(center);
    const radius = Math.max(size.x, size.y, size.z) * 0.75 + 1;
    const dist = radius / Math.sin((this.camera.fov * Math.PI) / 360);
    const { dir, up } = this.presetVectors(this.preset);
    this.camera.up.copy(up);
    this.camera.position.copy(center.clone().add(dir.clone().normalize().multiplyScalar(dist)));
    this.controls.target.copy(center);
    this.controls.update();
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

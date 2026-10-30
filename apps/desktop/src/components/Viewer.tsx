import { useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { PartMesh } from "../api";
import type { Units } from "../settings";

export type DisplayMode = "shaded" | "edges" | "wire" | "xray";
export type ViewName = "front" | "back" | "left" | "right" | "top" | "bottom" | "iso";
export type V3 = [number, number, number];

export interface SectionState {
  on: boolean;
  axis: "x" | "y" | "z";
  at: number;
}

export interface MeasureResult {
  a: V3;
  b: V3;
  dist: number;
  d: V3;
}

export interface ViewerHandle {
  fit(): void;
  view(name: ViewName): void;
  clearMeasure(): void;
}

/** Shared between two viewers so their cameras move together. */
export interface CameraLink {
  pos: THREE.Vector3 | null;
  target: THREE.Vector3 | null;
  rev: number;
  src: string;
}

export interface ViewerColors {
  viewer: string;
  plate: string;
  grid: string;
  gridMajor: string;
  accent: string;
  accentText: string;
  text: string;
  muted: string;
  danger: string;
}

interface Props {
  meshes: PartMesh[];
  ghost?: PartMesh[];
  hidden?: Set<string>;
  exploded?: boolean;
  display: DisplayMode;
  ortho: boolean;
  plate: boolean;
  bed: V3;
  measuring?: boolean;
  section?: SectionState;
  colors: ViewerColors;
  frameKey: string;
  units: Units;
  link?: CameraLink;
  onMeasure?: (r: MeasureResult | null) => void;
  onBounds?: (min: V3, max: V3) => void;
  handleRef?: React.MutableRefObject<ViewerHandle | null>;
  showCube?: boolean;
  label?: string;
  measureLabel?: string;
}

const DIRS: Record<ViewName, V3> = {
  front: [0, -1, 0],
  back: [0, 1, 0],
  left: [-1, 0, 0],
  right: [1, 0, 0],
  top: [0, -0.0005, 1],
  bottom: [0, -0.0005, -1],
  iso: [1, -1, 0.82],
};

const FACES: { name: ViewName; label: string; n: V3; r: V3; u: V3 }[] = [
  { name: "front", label: "FRONT", n: [0, -1, 0], r: [1, 0, 0], u: [0, 0, 1] },
  { name: "back", label: "BACK", n: [0, 1, 0], r: [-1, 0, 0], u: [0, 0, 1] },
  { name: "right", label: "RIGHT", n: [1, 0, 0], r: [0, 1, 0], u: [0, 0, 1] },
  { name: "left", label: "LEFT", n: [-1, 0, 0], r: [0, -1, 0], u: [0, 0, 1] },
  { name: "top", label: "TOP", n: [0, 0, 1], r: [1, 0, 0], u: [0, 1, 0] },
  { name: "bottom", label: "BOTTOM", n: [0, 0, -1], r: [1, 0, 0], u: [0, -1, 0] },
];

function luminance(hex: string): number {
  const c = new THREE.Color(hex);
  return 0.3 * c.r + 0.59 * c.g + 0.11 * c.b;
}

export function partGeometry(p: PartMesh): THREE.BufferGeometry {
  const geo = new THREE.BufferGeometry();
  geo.setAttribute("position", new THREE.Float32BufferAttribute(p.positions, 3));
  geo.setIndex(p.indices);
  geo.computeBoundingBox();
  return geo;
}

interface S {
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  pc: THREE.PerspectiveCamera;
  oc: THREE.OrthographicCamera;
  ortho: boolean;
  orthoH: number;
  controls: OrbitControls;
  model: THREE.Group;
  ghost: THREE.Group;
  plate: THREE.Group;
  measure: THREE.Group;
  sectionHelper: THREE.Mesh;
  clip: THREE.Plane;
  pickables: THREE.Mesh[];
  mA: THREE.Vector3 | null;
  mB: THREE.Vector3 | null;
  hover: THREE.Mesh;
  anim: { p0: THREE.Vector3; p1: THREE.Vector3; t0v: THREE.Vector3; t1v: THREE.Vector3; start: number; dur: number } | null;
  framed: string;
  id: string;
  linkSeen: number;
  applyingLink: boolean;
  w: number;
  h: number;
}

export function Viewer(props: Props) {
  const host = useRef<HTMLDivElement>(null);
  const cubeRef = useRef<HTMLDivElement>(null);
  const labelRef = useRef<HTMLDivElement>(null);
  const st = useRef<S | null>(null);
  const propsRef = useRef(props);
  propsRef.current = props;

  // --- set up once ------------------------------------------------------------
  useEffect(() => {
    const el = host.current!;
    const renderer = new THREE.WebGLRenderer({ antialias: true });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.localClippingEnabled = true;
    el.appendChild(renderer.domElement);
    renderer.domElement.style.display = "block";
    const scene = new THREE.Scene();
    scene.add(new THREE.HemisphereLight(0xffffff, 0x6a7280, 1.25));
    const pc = new THREE.PerspectiveCamera(35, 1, 0.5, 50000);
    const oc = new THREE.OrthographicCamera(-100, 100, 100, -100, -50000, 50000);
    for (const cam of [pc, oc]) {
      cam.up.set(0, 0, 1);
      const head = new THREE.DirectionalLight(0xffffff, 1.5);
      head.position.set(-0.4, 0.7, 1);
      cam.add(head);
      scene.add(cam);
    }
    pc.position.set(220, -280, 200);
    const controls = new OrbitControls(pc, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.18;
    controls.zoomToCursor = true;
    controls.mouseButtons = { LEFT: THREE.MOUSE.ROTATE, MIDDLE: THREE.MOUSE.PAN, RIGHT: THREE.MOUSE.PAN };
    const model = new THREE.Group();
    const ghost = new THREE.Group();
    const plate = new THREE.Group();
    const measure = new THREE.Group();
    measure.renderOrder = 10;
    const sectionHelper = new THREE.Mesh(
      new THREE.PlaneGeometry(1, 1),
      new THREE.MeshBasicMaterial({ transparent: true, opacity: 0.12, side: THREE.DoubleSide, depthWrite: false }),
    );
    sectionHelper.visible = false;
    const hover = new THREE.Mesh(new THREE.SphereGeometry(1, 16, 12), new THREE.MeshBasicMaterial({ depthTest: false, transparent: true, opacity: 0.85 }));
    hover.visible = false;
    hover.renderOrder = 11;
    scene.add(plate, model, ghost, measure, sectionHelper, hover);
    const s: S = {
      renderer, scene, pc, oc, ortho: false, orthoH: 100, controls, model, ghost, plate, measure, sectionHelper,
      clip: new THREE.Plane(new THREE.Vector3(0, 1, 0), 0), pickables: [], mA: null, mB: null, hover, anim: null,
      framed: "", id: Math.random().toString(36).slice(2), linkSeen: -1, applyingLink: false, w: 1, h: 1,
    };
    st.current = s;

    const cam = () => (s.ortho ? s.oc : s.pc);
    const resize = () => {
      s.w = Math.max(el.clientWidth, 1);
      s.h = Math.max(el.clientHeight, 1);
      renderer.setSize(s.w, s.h);
      pc.aspect = s.w / s.h;
      pc.updateProjectionMatrix();
      const a = s.w / s.h;
      oc.left = -s.orthoH * a;
      oc.right = s.orthoH * a;
      oc.top = s.orthoH;
      oc.bottom = -s.orthoH;
      oc.updateProjectionMatrix();
    };
    const ro = new ResizeObserver(resize);
    ro.observe(el);
    resize();

    controls.addEventListener("change", () => {
      const link = propsRef.current.link;
      if (!link || s.applyingLink) return;
      link.pos = cam().position.clone();
      link.target = controls.target.clone();
      link.rev += 1;
      link.src = s.id;
      s.linkSeen = link.rev;
    });

    // measuring: click (no drag) on the model
    const ray = new THREE.Raycaster();
    let down: { x: number; y: number } | null = null;
    const ndc = (e: PointerEvent) => {
      const r = renderer.domElement.getBoundingClientRect();
      return new THREE.Vector2(((e.clientX - r.left) / r.width) * 2 - 1, -((e.clientY - r.top) / r.height) * 2 + 1);
    };
    const pick = (e: PointerEvent): THREE.Vector3 | null => {
      ray.setFromCamera(ndc(e), cam());
      const sec = propsRef.current.section;
      const hits = ray.intersectObjects(s.pickables, false).filter((h) => !(sec?.on) || s.clip.distanceToPoint(h.point) >= -1e-6);
      const hit = hits[0];
      if (!hit) return null;
      // snap to the nearest corner of the triangle when close on screen
      const mesh = hit.object as THREE.Mesh;
      const pos = mesh.geometry.getAttribute("position");
      let best = hit.point.clone();
      let bestPx = 12;
      if (hit.face) {
        for (const i of [hit.face.a, hit.face.b, hit.face.c]) {
          const v = new THREE.Vector3().fromBufferAttribute(pos, i).applyMatrix4(mesh.matrixWorld);
          const pv = v.clone().project(cam());
          const ph = hit.point.clone().project(cam());
          const px = Math.hypot(((pv.x - ph.x) * s.w) / 2, ((pv.y - ph.y) * s.h) / 2);
          if (px < bestPx) {
            bestPx = px;
            best = v;
          }
        }
      }
      return best;
    };
    const onDown = (e: PointerEvent) => {
      down = { x: e.clientX, y: e.clientY };
    };
    const onUp = (e: PointerEvent) => {
      if (!down || !propsRef.current.measuring || e.button !== 0) return;
      const moved = Math.hypot(e.clientX - down.x, e.clientY - down.y);
      down = null;
      if (moved > 4) return;
      const p = pick(e);
      if (!p) return;
      if (!s.mA || s.mB) {
        s.mA = p;
        s.mB = null;
        propsRef.current.onMeasure?.(null);
      } else {
        s.mB = p;
        const d = s.mB.clone().sub(s.mA);
        propsRef.current.onMeasure?.({ a: s.mA.toArray() as V3, b: s.mB.toArray() as V3, dist: d.length(), d: [Math.abs(d.x), Math.abs(d.y), Math.abs(d.z)] });
      }
      drawMeasure();
    };
    const onMove = (e: PointerEvent) => {
      if (!propsRef.current.measuring || e.buttons) {
        hover.visible = false;
        return;
      }
      const p = pick(e);
      hover.visible = !!p;
      if (p) hover.position.copy(p);
    };
    renderer.domElement.addEventListener("pointerdown", onDown);
    renderer.domElement.addEventListener("pointerup", onUp);
    renderer.domElement.addEventListener("pointermove", onMove);

    const drawMeasure = () => {
      measure.clear();
      const c = propsRef.current.colors;
      const mat = new THREE.MeshBasicMaterial({ color: c.accentText, depthTest: false });
      for (const p of [s.mA, s.mB]) {
        if (!p) continue;
        const dot = new THREE.Mesh(new THREE.SphereGeometry(1, 16, 12), mat);
        dot.position.copy(p);
        dot.userData.dot = true;
        dot.renderOrder = 10;
        measure.add(dot);
      }
      if (s.mA && s.mB) {
        const line = new THREE.Line(new THREE.BufferGeometry().setFromPoints([s.mA, s.mB]), new THREE.LineBasicMaterial({ color: c.accentText, depthTest: false }));
        line.renderOrder = 10;
        measure.add(line);
      }
    };

    // --- frame loop -----------------------------------------------------------
    let raf = 0;
    const tmp = new THREE.Vector3();
    const tick = () => {
      raf = requestAnimationFrame(tick);
      const c = cam();
      if (s.anim) {
        const t = Math.min(1, (performance.now() - s.anim.start) / s.anim.dur);
        const k = t < 0.5 ? 2 * t * t : 1 - (-2 * t + 2) ** 2 / 2;
        c.position.lerpVectors(s.anim.p0, s.anim.p1, k);
        controls.target.lerpVectors(s.anim.t0v, s.anim.t1v, k);
        if (t >= 1) s.anim = null;
      }
      const link = propsRef.current.link;
      if (link && link.rev !== s.linkSeen && link.src !== s.id && link.pos && link.target) {
        s.applyingLink = true;
        c.position.copy(link.pos);
        controls.target.copy(link.target);
        controls.update();
        s.applyingLink = false;
        s.linkSeen = link.rev;
      }
      controls.update();
      // keep measure dots a constant size on screen
      const dist = c.position.distanceTo(controls.target);
      const px = s.ortho ? (2 * s.orthoH) / s.oc.zoom / s.h : (2 * dist * Math.tan((s.pc.fov * Math.PI) / 360)) / s.h;
      for (const o of measure.children) if (o.userData.dot) o.scale.setScalar(px * 5);
      hover.scale.setScalar(px * 4);
      renderer.render(scene, c);
      // view cube follows the camera
      c.updateMatrixWorld();
      const R = new THREE.Matrix4().extractRotation(c.matrixWorldInverse);
      if (cubeRef.current) {
        const faces = cubeRef.current.children;
        FACES.forEach((f, i) => {
          const tr = (v: V3) => {
            tmp.set(v[0], v[1], v[2]).applyMatrix4(R);
            return [tmp.x, -tmp.y, tmp.z];
          };
          const c0 = tr(f.r);
          const c1 = tr([-f.u[0], -f.u[1], -f.u[2]]);
          const c2 = tr(f.n);
          const t = tr([f.n[0] * 34, f.n[1] * 34, f.n[2] * 34]);
          (faces[i] as HTMLElement).style.transform = `matrix3d(${c0[0]},${c0[1]},${c0[2]},0,${c1[0]},${c1[1]},${c1[2]},0,${c2[0]},${c2[1]},${c2[2]},0,${t[0]},${t[1]},${t[2]},1)`;
        });
      }
      // measure label
      if (labelRef.current) {
        if (s.mA && s.mB) {
          const mid = s.mA.clone().add(s.mB).multiplyScalar(0.5).project(c);
          labelRef.current.style.display = "block";
          labelRef.current.style.transform = `translate(${((mid.x + 1) / 2) * s.w}px, ${((1 - mid.y) / 2) * s.h}px) translate(-50%, -140%)`;
        } else {
          labelRef.current.style.display = "none";
        }
      }
    };
    tick();

    const handle: ViewerHandle = {
      fit: () => frame(null),
      view: (name) => frame(DIRS[name]),
      clearMeasure: () => {
        s.mA = s.mB = null;
        drawMeasure();
        propsRef.current.onMeasure?.(null);
      },
    };
    const frame = (dirIn: V3 | null) => {
      const box = new THREE.Box3().setFromObject(model);
      if (box.isEmpty()) return;
      const center = box.getCenter(new THREE.Vector3());
      const r = Math.max(box.getSize(new THREE.Vector3()).length() / 2, 1);
      const c = cam();
      const dir = dirIn ? new THREE.Vector3(...dirIn).normalize() : c.position.clone().sub(controls.target).normalize();
      // fit the bounding sphere in the narrower of the two view angles, with
      // room for the overlays (toolbar, view cube)
      const vHalf = (s.pc.fov * Math.PI) / 360;
      const aspect = s.w / s.h;
      const half = Math.min(vHalf, Math.atan(Math.tan(vHalf) * aspect));
      const margin = 1.38;
      const dist = (r / Math.sin(half)) * margin;
      s.orthoH = (r * margin) / Math.min(1, aspect);
      s.oc.zoom = 1;
      resize();
      s.pc.near = Math.max(0.1, dist / 400);
      s.pc.far = dist * 60;
      s.pc.updateProjectionMatrix();
      s.anim = { p0: c.position.clone(), p1: center.clone().add(dir.multiplyScalar(dist)), t0v: controls.target.clone(), t1v: center, start: performance.now(), dur: 380 };
    };
    if (propsRef.current.handleRef) propsRef.current.handleRef.current = handle;
    (s as unknown as { frame: typeof frame }).frame = frame;
    (s as unknown as { drawMeasure: typeof drawMeasure }).drawMeasure = drawMeasure;

    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      controls.dispose();
      renderer.domElement.removeEventListener("pointerdown", onDown);
      renderer.domElement.removeEventListener("pointerup", onUp);
      renderer.domElement.removeEventListener("pointermove", onMove);
      renderer.dispose();
      el.removeChild(renderer.domElement);
      st.current = null;
    };
  }, []);

  // --- model -------------------------------------------------------------------
  const { meshes, ghost, hidden, exploded, display, section, colors } = props;
  useEffect(() => {
    const s = st.current;
    if (!s) return;
    const dispose = (g: THREE.Group) => {
      g.traverse((o) => {
        const m = o as THREE.Mesh;
        if (m.geometry) m.geometry.dispose();
        const mat = m.material as THREE.Material | THREE.Material[] | undefined;
        if (Array.isArray(mat)) mat.forEach((x) => x.dispose());
        else mat?.dispose();
      });
      g.clear();
      g.position.set(0, 0, 0);
    };
    dispose(s.model);
    dispose(s.ghost);
    s.pickables = [];
    const clipping = section?.on ? [s.clip] : [];
    const geos = meshes.map((p) => ({ p, geo: partGeometry(p) }));
    const all = new THREE.Box3();
    geos.forEach(({ geo }) => all.union(geo.boundingBox!));
    const center = all.getCenter(new THREE.Vector3());
    const size = all.getSize(new THREE.Vector3());
    for (const { p, geo } of geos) {
      const part = new THREE.Group();
      part.visible = !hidden?.has(p.id);
      const flat = geo.toNonIndexed();
      flat.computeVertexNormals();
      const color = new THREE.Color(p.color);
      const faces = new THREE.Mesh(
        flat,
        new THREE.MeshStandardMaterial({
          color,
          roughness: 0.55,
          metalness: 0.04,
          polygonOffset: true,
          polygonOffsetFactor: 1,
          polygonOffsetUnits: 1,
          clippingPlanes: clipping,
          transparent: display === "xray",
          opacity: display === "xray" ? 0.32 : 1,
          depthWrite: display !== "xray",
          visible: display !== "wire",
        }),
      );
      part.add(faces);
      if (part.visible) s.pickables.push(faces);
      if (section?.on && display !== "wire") {
        part.add(new THREE.Mesh(flat, new THREE.MeshBasicMaterial({ color: colors.danger, side: THREE.BackSide, clippingPlanes: clipping })));
      }
      if (display !== "shaded") {
        const lineColor = display === "wire" ? colors.text : luminance(p.color) < 0.3 ? 0x9aa0aa : 0x1f232a;
        const edges = new THREE.LineSegments(
          new THREE.EdgesGeometry(geo, 25),
          new THREE.LineBasicMaterial({ color: lineColor, clippingPlanes: clipping, transparent: display === "xray", opacity: display === "xray" ? 0.6 : 1 }),
        );
        part.add(edges);
      }
      if (exploded) {
        const d = geo.boundingBox!.getCenter(new THREE.Vector3()).sub(center);
        if (d.lengthSq() > 1e-6) {
          d.normalize();
          const ext = Math.abs(d.x * size.x) + Math.abs(d.y * size.y) + Math.abs(d.z * size.z);
          part.position.copy(d.multiplyScalar(ext * 0.12 + 10));
        }
      }
      s.model.add(part);
    }
    for (const p of ghost ?? []) {
      const geo = partGeometry(p);
      s.ghost.add(new THREE.Mesh(geo, new THREE.MeshBasicMaterial({ color: colors.muted, transparent: true, opacity: 0.16, depthWrite: false })));
      s.ghost.add(new THREE.LineSegments(new THREE.EdgesGeometry(geo, 25), new THREE.LineDashedMaterial({ color: colors.muted, dashSize: 3, gapSize: 2 })).computeLineDistances());
    }
    const raw = new THREE.Box3().setFromObject(s.model);
    if (!raw.isEmpty() && raw.min.z < 0) s.model.position.z = -raw.min.z;
    const box = new THREE.Box3().setFromObject(s.model);
    if (!box.isEmpty()) {
      propsRef.current.onBounds?.(box.min.toArray() as V3, box.max.toArray() as V3);
      if (s.framed !== propsRef.current.frameKey) {
        s.framed = propsRef.current.frameKey;
        (s as unknown as { frame: (d: V3 | null) => void }).frame(DIRS.iso);
      }
    }
  }, [meshes, ghost, hidden, exploded, display, section?.on, colors]);

  // --- section plane -------------------------------------------------------------
  useEffect(() => {
    const s = st.current;
    if (!s || !section) return;
    const n = section.axis === "x" ? new THREE.Vector3(-1, 0, 0) : section.axis === "y" ? new THREE.Vector3(0, 1, 0) : new THREE.Vector3(0, 0, -1);
    const sign = section.axis === "y" ? -1 : 1;
    s.clip.set(n, sign * section.at);
    const box = new THREE.Box3().setFromObject(s.model);
    s.sectionHelper.visible = section.on && !box.isEmpty();
    if (s.sectionHelper.visible) {
      const size = box.getSize(new THREE.Vector3()).multiplyScalar(1.15);
      const c = box.getCenter(new THREE.Vector3());
      const h = s.sectionHelper;
      (h.material as THREE.MeshBasicMaterial).color.set(colors.accent);
      h.rotation.set(0, 0, 0);
      if (section.axis === "x") {
        h.scale.set(size.y, size.z, 1);
        h.rotation.set(Math.PI / 2, Math.PI / 2, 0);
        h.position.set(section.at, c.y, c.z);
      } else if (section.axis === "y") {
        h.scale.set(size.x, size.z, 1);
        h.rotation.set(Math.PI / 2, 0, 0);
        h.position.set(c.x, section.at, c.z);
      } else {
        h.scale.set(size.x, size.y, 1);
        h.position.set(c.x, c.y, section.at);
      }
    }
  }, [section?.on, section?.axis, section?.at, meshes, colors]);

  // --- plate / grid ------------------------------------------------------------------
  const { plate, bed } = props;
  useEffect(() => {
    const s = st.current;
    if (!s) return;
    s.plate.clear();
    s.renderer.setClearColor(colors.viewer);
    const [bx, by] = plate ? [bed[0], bed[1]] : [400, 400];
    if (plate) {
      const base = new THREE.Mesh(new THREE.PlaneGeometry(bx, by), new THREE.MeshBasicMaterial({ color: colors.plate }));
      base.position.z = -0.6;
      s.plate.add(base);
    }
    const pts: number[] = [];
    const major: number[] = [];
    const step = 10;
    for (let x = -bx / 2; x <= bx / 2 + 1e-6; x += step) {
      const isMajor = Math.abs(Math.round(x / 50) * 50 - x) < 1e-6;
      (isMajor ? major : pts).push(x, -by / 2, -0.4, x, by / 2, -0.4);
    }
    for (let y = -by / 2; y <= by / 2 + 1e-6; y += step) {
      const isMajor = Math.abs(Math.round(y / 50) * 50 - y) < 1e-6;
      (isMajor ? major : pts).push(-bx / 2, y, -0.4, bx / 2, y, -0.4);
    }
    const lines = (arr: number[], color: string) => {
      const g = new THREE.BufferGeometry();
      g.setAttribute("position", new THREE.Float32BufferAttribute(arr, 3));
      return new THREE.LineSegments(g, new THREE.LineBasicMaterial({ color, transparent: !plate, opacity: plate ? 1 : 0.7 }));
    };
    s.plate.add(lines(pts, colors.grid), lines(major, colors.gridMajor));
    if (plate) {
      const edge = new THREE.LineLoop(
        new THREE.BufferGeometry().setFromPoints([new THREE.Vector3(-bx / 2, -by / 2, -0.3), new THREE.Vector3(bx / 2, -by / 2, -0.3), new THREE.Vector3(bx / 2, by / 2, -0.3), new THREE.Vector3(-bx / 2, by / 2, -0.3)]),
        new THREE.LineBasicMaterial({ color: colors.accent }),
      );
      s.plate.add(edge);
    }
    const axes = new THREE.AxesHelper(30);
    axes.position.z = -0.2;
    s.plate.add(axes);
    (s.hover.material as THREE.MeshBasicMaterial).color.set(colors.accentText);
    (s as unknown as { drawMeasure: () => void }).drawMeasure();
  }, [plate, bed[0], bed[1], colors]);

  // --- projection -------------------------------------------------------------------------
  const { ortho } = props;
  useEffect(() => {
    const s = st.current;
    if (!s || s.ortho === ortho) return;
    const from = s.ortho ? s.oc : s.pc;
    const to = ortho ? s.oc : s.pc;
    const target = s.controls.target;
    const dist = from.position.distanceTo(target);
    const tanHalf = Math.tan((s.pc.fov * Math.PI) / 360);
    if (ortho) {
      s.orthoH = dist * tanHalf;
      s.oc.zoom = 1;
      to.position.copy(from.position);
    } else {
      const h = s.orthoH / s.oc.zoom;
      const dir = from.position.clone().sub(target).normalize();
      to.position.copy(target.clone().add(dir.multiplyScalar(h / tanHalf)));
    }
    to.up.set(0, 0, 1);
    s.ortho = ortho;
    s.controls.object = to;
    const a = s.w / s.h;
    s.oc.left = -s.orthoH * a;
    s.oc.right = s.orthoH * a;
    s.oc.top = s.orthoH;
    s.oc.bottom = -s.orthoH;
    s.oc.updateProjectionMatrix();
    s.controls.update();
  }, [ortho]);

  useEffect(() => {
    if (props.handleRef && st.current) {
      // keep the handle attached if the parent swaps refs
    }
    if (!props.measuring && st.current) {
      st.current.hover.visible = false;
    }
  }, [props.measuring, props.handleRef]);

  const showCube = props.showCube ?? true;
  return (
    <div className="viewer-wrap">
      <div className="viewer-canvas" ref={host} style={{ cursor: props.measuring ? "crosshair" : undefined }} />
      {props.label && <div className="viewer-label">{props.label}</div>}
      <div className="measure-label" ref={labelRef}>
        {props.measureLabel}
      </div>
      {showCube && (
        <div className="viewcube" aria-label="View cube">
          <div className="viewcube-inner" ref={cubeRef}>
            {FACES.map((f) => (
              <button key={f.name} type="button" className="viewcube-face" onClick={() => props.handleRef?.current?.view(f.name)} aria-label={`View from ${f.name}`}>
                {f.label}
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

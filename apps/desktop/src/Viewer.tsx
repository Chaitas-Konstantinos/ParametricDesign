import { useEffect, useRef } from "react";
import * as THREE from "three";
import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";
import type { PartMesh } from "./engine";

interface Props {
  meshes: PartMesh[];
  exploded: boolean;
  /** Changes when a different design is loaded, so the camera re-frames it. */
  designKey: string;
}

interface Scene {
  renderer: THREE.WebGLRenderer;
  scene: THREE.Scene;
  camera: THREE.PerspectiveCamera;
  controls: OrbitControls;
  group: THREE.Group;
  grid: THREE.Group;
  framedKey: string;
}

function makeGrid(size: number): THREE.Group {
  const g = new THREE.Group();
  const step = size > 400 ? 50 : 10;
  const n = Math.ceil(size / step);
  const fine = new THREE.GridHelper(n * step, n, 0xc5cad3, 0xdfe3e8);
  fine.rotation.x = Math.PI / 2; // grid in the XY plane (Z up)
  g.add(fine);
  const axes = new THREE.AxesHelper(step * 2);
  g.add(axes);
  return g;
}

function buildMeshes(parts: PartMesh[], exploded: boolean): THREE.Group {
  const group = new THREE.Group();
  const boxes = parts.map((p) => {
    const geo = new THREE.BufferGeometry();
    geo.setAttribute("position", new THREE.Float32BufferAttribute(p.positions, 3));
    geo.setIndex(p.indices);
    geo.computeBoundingBox();
    return { p, geo, box: geo.boundingBox! };
  });
  const all = new THREE.Box3();
  boxes.forEach((b) => all.union(b.box));
  const center = all.getCenter(new THREE.Vector3());
  const size = all.getSize(new THREE.Vector3());
  for (const { p, geo, box } of boxes) {
    const flat = geo.toNonIndexed();
    flat.computeVertexNormals();
    const color = new THREE.Color(p.color);
    const mesh = new THREE.Mesh(
      flat,
      new THREE.MeshStandardMaterial({ color, roughness: 0.65, metalness: 0.05, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 }),
    );
    const lum = 0.3 * color.r + 0.59 * color.g + 0.11 * color.b;
    const edges = new THREE.LineSegments(new THREE.EdgesGeometry(geo, 25), new THREE.LineBasicMaterial({ color: lum < 0.3 ? 0x9aa0aa : 0x23262c }));
    const part = new THREE.Group();
    part.name = p.id;
    part.add(mesh, edges);
    if (exploded) {
      const d = box.getCenter(new THREE.Vector3()).sub(center);
      if (d.lengthSq() > 1e-6) {
        d.normalize();
        const ext = Math.abs(d.x * size.x) + Math.abs(d.y * size.y) + Math.abs(d.z * size.z);
        part.position.copy(d.multiplyScalar(ext * 0.12 + 10));
      }
    }
    group.add(part);
  }
  return group;
}

export function Viewer({ meshes, exploded, designKey }: Props) {
  const host = useRef<HTMLDivElement>(null);
  const s = useRef<Scene | null>(null);

  useEffect(() => {
    const el = host.current!;
    const renderer = new THREE.WebGLRenderer({ antialias: true, preserveDrawingBuffer: true });
    renderer.setPixelRatio(window.devicePixelRatio);
    renderer.setClearColor(0xeef0f4);
    el.appendChild(renderer.domElement);
    const scene = new THREE.Scene();
    scene.add(new THREE.HemisphereLight(0xffffff, 0x8890a0, 1.6));
    const sun = new THREE.DirectionalLight(0xffffff, 1.6);
    sun.position.set(-1, -2, 3);
    scene.add(sun);
    const camera = new THREE.PerspectiveCamera(35, 1, 0.5, 20000);
    camera.up.set(0, 0, 1);
    camera.position.set(200, -260, 180);
    const controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    const group = new THREE.Group();
    const grid = makeGrid(256);
    scene.add(group, grid);
    s.current = { renderer, scene, camera, controls, group, grid, framedKey: "" };
    const resize = () => {
      const { clientWidth: w, clientHeight: h } = el;
      renderer.setSize(w, h);
      camera.aspect = w / Math.max(h, 1);
      camera.updateProjectionMatrix();
    };
    const ro = new ResizeObserver(resize);
    ro.observe(el);
    resize();
    let raf = 0;
    const tick = () => {
      controls.update();
      renderer.render(scene, camera);
      raf = requestAnimationFrame(tick);
    };
    tick();
    return () => {
      cancelAnimationFrame(raf);
      ro.disconnect();
      controls.dispose();
      renderer.dispose();
      el.removeChild(renderer.domElement);
      s.current = null;
    };
  }, []);

  useEffect(() => {
    const st = s.current;
    if (!st) return;
    st.scene.remove(st.group);
    st.group.traverse((o) => {
      if (o instanceof THREE.Mesh || o instanceof THREE.LineSegments) {
        o.geometry.dispose();
        (o.material as THREE.Material).dispose();
      }
    });
    st.group = buildMeshes(meshes, exploded);
    st.scene.add(st.group);
    // keep everything on or above the bed (exploding can push parts below it)
    const raw = new THREE.Box3().setFromObject(st.group);
    if (!raw.isEmpty() && raw.min.z < 0) st.group.position.z = -raw.min.z;
    const box = new THREE.Box3().setFromObject(st.group);
    if (box.isEmpty()) return;
    const size = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3());
    st.scene.remove(st.grid);
    st.grid = makeGrid(Math.max(256, size.x * 1.4, size.y * 1.4));
    st.scene.add(st.grid);
    if (st.framedKey !== designKey) {
      // frame the model from the front-right-top
      st.framedKey = designKey;
      const r = size.length() / 2;
      const dist = r / Math.sin((st.camera.fov * Math.PI) / 360) * 1.15;
      const dir = new THREE.Vector3(0.75, -1, 0.7).normalize();
      st.controls.target.copy(center);
      st.camera.position.copy(center.clone().add(dir.multiplyScalar(dist)));
      st.camera.near = dist / 100;
      st.camera.far = dist * 100;
      st.camera.updateProjectionMatrix();
    }
  }, [meshes, exploded, designKey]);

  return <div className="viewer" ref={host} />;
}

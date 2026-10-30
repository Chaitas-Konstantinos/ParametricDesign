// Library thumbnails: an offscreen iso render with a transparent background.
import * as THREE from "three";
import type { DesignInfo, PartMesh } from "./api";
import { partGeometry } from "./components/Viewer";

let renderer: THREE.WebGLRenderer | null = null;

export function renderThumbnail(meshes: PartMesh[]): { png: string; info: DesignInfo } | null {
  if (!meshes.length) return null;
  if (!renderer) {
    renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true, preserveDrawingBuffer: true });
    renderer.setPixelRatio(1);
    renderer.setSize(480, 360, false);
  }
  renderer.setClearColor(0x000000, 0);
  const scene = new THREE.Scene();
  scene.add(new THREE.HemisphereLight(0xffffff, 0x6a7280, 1.3));
  const cam = new THREE.PerspectiveCamera(30, 480 / 360, 0.5, 50000);
  cam.up.set(0, 0, 1);
  const head = new THREE.DirectionalLight(0xffffff, 1.5);
  head.position.set(-0.4, 0.7, 1);
  cam.add(head);
  scene.add(cam);
  const box = new THREE.Box3();
  const disposables: { dispose(): void }[] = [];
  for (const p of meshes) {
    const geo = partGeometry(p);
    box.union(geo.boundingBox!);
    const flat = geo.toNonIndexed();
    flat.computeVertexNormals();
    const mat = new THREE.MeshStandardMaterial({ color: p.color, roughness: 0.55, metalness: 0.04, polygonOffset: true, polygonOffsetFactor: 1, polygonOffsetUnits: 1 });
    const edges = new THREE.EdgesGeometry(geo, 25);
    const emat = new THREE.LineBasicMaterial({ color: 0x1f232a, transparent: true, opacity: 0.55 });
    scene.add(new THREE.Mesh(flat, mat), new THREE.LineSegments(edges, emat));
    disposables.push(geo, flat, mat, edges, emat);
  }
  const size = box.getSize(new THREE.Vector3());
  const center = box.getCenter(new THREE.Vector3());
  const r = Math.max(size.length() / 2, 1);
  const dir = new THREE.Vector3(1, -1, 0.82).normalize();
  cam.position.copy(center.clone().add(dir.multiplyScalar((r / Math.sin((30 * Math.PI) / 360)) * 1.02)));
  cam.lookAt(center);
  renderer.render(scene, cam);
  const png = renderer.domElement.toDataURL("image/png");
  disposables.forEach((d) => d.dispose());
  return { png, info: { size: [size.x, size.y, size.z].map((v) => Math.round(v * 10) / 10) as [number, number, number], parts: meshes.length } };
}

// Everything the UI needs from outside: the engine, the library and settings.
// Desktop: native Rust through Tauri commands. Browser (dev / future website):
// the same engine as WebAssembly, the library in local storage.
import type { Settings } from "./settings";
import { slug } from "./settings";

export type ParamValue = number | boolean | string;
export type Params = Record<string, ParamValue>;

export interface Issue {
  severity: "error" | "warning";
  code: string;
  path: string;
  message: string;
  hint?: string;
  when?: string;
}

export interface ParamInfo {
  name: string;
  type: "number" | "integer" | "boolean" | "choice" | "color";
  value: ParamValue;
  default: ParamValue;
  min?: number;
  max?: number;
  step?: number;
  unit?: string;
  label?: string;
  description?: string;
  group?: string;
  advanced: boolean;
  options?: ParamValue[];
}

export interface PartSummary {
  id: string;
  name?: string;
  color: string;
  size: [number, number, number];
  min: [number, number, number];
  max: [number, number, number];
  volume_mm3: number;
  triangles: number;
}

export interface Report {
  ok: boolean;
  name: string;
  errors: Issue[];
  warnings: Issue[];
  parts: PartSummary[];
  params: ParamInfo[];
}

export interface PartMesh {
  id: string;
  name?: string;
  color: string;
  positions: number[];
  indices: number[];
}

export interface Evaluation {
  report: Report;
  meshes: PartMesh[];
}

export interface EvalOpts {
  printer?: { bed: number[]; nozzle: number };
}

export interface DesignInfo {
  size?: [number, number, number];
  parts?: number;
}

export interface DesignMeta {
  id: string;
  name: string;
  description: string;
  category: string;
  tags: string[];
  favorite: boolean;
  collections: string[];
  created: number;
  modified: number;
  opened?: number | null;
  source: string;
  isNew: boolean;
  trashed?: number | null;
  currentVersion: number;
  info: DesignInfo;
}

export interface Summary extends DesignMeta {
  versionCount: number;
  presetCount: number;
  pendingCount: number;
  thumbnail?: string | null;
}

export interface VersionInfo {
  file: string;
  num: number;
  created: number;
  author: string;
  message: string;
  params: Params;
  rejected: boolean;
}

export interface VersionFile extends VersionInfo {
  design: unknown;
}

export interface Preset {
  name: string;
  params: Params;
}

export interface Pending {
  file: string;
  created: number;
  message: string;
}

export interface Detail {
  meta: DesignMeta;
  versions: VersionInfo[];
  presets: Preset[];
  pending: Pending[];
}

export interface Collection {
  id: string;
  name: string;
}

export interface InboxEvent {
  kind: "new" | "update";
  id: string;
  name: string;
  message: string;
}

export interface CurrentState {
  open: boolean;
  designId?: string;
  name?: string;
  version?: number;
  unsaved?: boolean;
  params?: Params;
  design?: unknown;
}

export interface SkillInfo {
  id: string;
  title: string;
  core: boolean;
  /** Skills in one group are alternatives (pick one connector type). */
  group?: string | null;
  requires: string[];
  summary: string;
}

export interface RequestPhoto {
  /** File name as the user added it. */
  name: string;
  caption: string;
  /** Downscaled image as a data URL. */
  dataUrl: string;
}

/** A design request: what to design, for which printer, with which skills. */
export interface DesignRequest {
  title: string;
  brief: string;
  optional: string[];
  photos: RequestPhoto[];
  printer?: { name: string; bed: number[]; nozzle: number } | null;
}

export interface PromptExport {
  prompt: string;
  fileName: string;
  /** What to send when the prompt goes into the chat as a file. */
  starter: string;
}

export interface Api {
  kind: "desktop" | "browser";
  evaluate(design: string, params: Params, opts: EvalOpts): Promise<Evaluation>;
  checkAll(design: string, params: Params, opts: EvalOpts): Promise<Report>;
  /** Asks where to save, writes the file; resolves to its name/path, or null if cancelled. */
  exportFile(design: string, params: Params, opts: EvalOpts, format: "stl" | "3mf", part: string | undefined, baseName: string): Promise<string | null>;

  getSettings(): Promise<Partial<Settings>>;
  saveSettings(s: Settings): Promise<void>;
  pickFolder(current?: string): Promise<string | null>;
  mcpServerPath(): Promise<string | null>;
  setCurrent(state: CurrentState): Promise<void>;
  onInbox(cb: (events: InboxEvent[]) => void): Promise<() => void>;
  /** cb returns true when the window may close. */
  onCloseRequested(cb: () => Promise<boolean>): Promise<() => void>;

  list(): Promise<Summary[]>;
  detail(id: string): Promise<Detail>;
  readVersion(id: string, file?: string): Promise<VersionFile>;
  create(design: string, params: Params, source: string, message: string): Promise<string>;
  saveVersion(id: string, design: string, params: Params, author: string, message: string, rejected?: boolean): Promise<VersionInfo>;
  updateMeta(id: string, patch: Partial<DesignMeta>): Promise<DesignMeta>;
  setPresets(id: string, presets: Preset[]): Promise<void>;
  setThumbnail(id: string, pngDataUrl: string): Promise<void>;
  deleteForever(id: string): Promise<void>;
  duplicate(id: string, file: string | null, name: string): Promise<string>;
  /** Joins `other`'s versions into `id` (renumbered by date); `other` goes to the trash. */
  combine(id: string, other: string): Promise<DesignMeta>;
  readPending(id: string, file: string): Promise<{ design: unknown; message: string; created: number }>;
  removePending(id: string, file: string): Promise<void>;
  collections(): Promise<Collection[]>;
  setCollections(c: Collection[]): Promise<void>;
  /** Asks for .pd.json / .3mf files and adds them; resolves to the new ids. */
  importFiles(): Promise<string[]>;
  addExamples(): Promise<number>;

  skillsCatalog(): Promise<SkillInfo[]>;
  /** Asks where to save the tailored skill (.zip); null if cancelled. */
  exportSkillBundle(req: DesignRequest): Promise<{ name: string; fileName: string; starter: string; path: string } | null>;
  /** Asks where to save one skill (.zip); resolves to the path, or null if cancelled. */
  exportSkill(id: string): Promise<string | null>;
  skillsPrompt(req: DesignRequest): Promise<PromptExport>;
  /** Asks where to save the prompt (.md); null if cancelled. */
  savePrompt(req: DesignRequest): Promise<(PromptExport & { path: string }) | null>;
  /** Hands the request to Claude through the MCP server (desktop app only). */
  sendRequest(req: DesignRequest): Promise<void>;
}

/** The request as the engine reads it (photos as data URLs). */
function requestJson(r: DesignRequest): string {
  return JSON.stringify({
    title: r.title,
    brief: r.brief,
    optional: r.optional,
    printer: r.printer ?? null,
    photos: r.photos.map((p) => ({ name: p.name, caption: p.caption, data: p.dataUrl })),
  });
}

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

// --- desktop ---------------------------------------------------------------------

async function desktopApi(): Promise<Api> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const { getCurrentWindow } = await import("@tauri-apps/api/window");
  const dialog = await import("@tauri-apps/plugin-dialog");
  const j = JSON.stringify;
  return {
    kind: "desktop",
    evaluate: (design, params, opts) => invoke("evaluate", { design, params: j(params), opts: j(opts) }),
    checkAll: (design, params, opts) => invoke("check_all", { design, params: j(params), opts: j(opts) }),
    async exportFile(design, params, opts, format, part, baseName) {
      const path = await dialog.save({ defaultPath: `${baseName}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
      if (!path) return null;
      await invoke("export_to_file", { design, params: j(params), opts: j(opts), format, part: part ?? null, path });
      return path;
    },
    getSettings: () => invoke("get_settings"),
    saveSettings: (settings) => invoke("save_settings", { settings }),
    async pickFolder(current) {
      const p = await dialog.open({ directory: true, multiple: false, defaultPath: current });
      return typeof p === "string" ? p : null;
    },
    mcpServerPath: () => invoke("mcp_server_path"),
    setCurrent: (state) => invoke("set_current", { state }),
    async onInbox(cb) {
      return listen<InboxEvent[]>("library-inbox", (e) => cb(e.payload));
    },
    async onCloseRequested(cb) {
      const w = getCurrentWindow();
      return w.onCloseRequested(async (event) => {
        event.preventDefault();
        if (await cb()) await w.destroy();
      });
    },
    list: () => invoke("library_list"),
    detail: (id) => invoke("library_detail", { id }),
    readVersion: (id, file) => invoke("library_read_version", { id, file: file ?? null }),
    create: (design, params, source, message) => invoke("library_create", { design, params, source, message }),
    saveVersion: (id, design, params, author, message, rejected = false) => invoke("library_save_version", { id, design, params, author, message, rejected }),
    updateMeta: (id, patch) => invoke("library_update_meta", { id, patch }),
    setPresets: (id, presets) => invoke("library_set_presets", { id, presets }),
    setThumbnail: (id, png) => invoke("library_set_thumbnail", { id, png }),
    deleteForever: (id) => invoke("library_delete", { id }),
    duplicate: (id, file, name) => invoke("library_duplicate", { id, file, name }),
    combine: (id, other) => invoke("library_combine", { id, other }),
    readPending: (id, file) => invoke("library_read_pending", { id, file }),
    removePending: (id, file) => invoke("library_remove_pending", { id, file }),
    collections: async () => ((await invoke("library_collections")) as Collection[]) ?? [],
    setCollections: (collections) => invoke("library_set_collections", { collections }),
    async importFiles() {
      const picked = await dialog.open({ multiple: true, filters: [{ name: "ParametricDesign designs", extensions: ["json", "3mf"] }] });
      const paths = Array.isArray(picked) ? picked : picked ? [picked] : [];
      const ids: string[] = [];
      for (const path of paths) ids.push(await invoke("library_import_file", { path }));
      return ids;
    },
    addExamples: () => invoke("library_add_examples"),
    skillsCatalog: () => invoke("skills_catalog"),
    async exportSkillBundle(req) {
      const request = requestJson(req);
      const suggested = (await invoke<{ fileName: string }>("skills_prompt", { request })).fileName.replace(/\.md$/, ".zip");
      const path = await dialog.save({ defaultPath: suggested, filters: [{ name: "Skill", extensions: ["zip"] }] });
      if (!path) return null;
      const info = await invoke<{ name: string; fileName: string; starter: string }>("skills_export_bundle", { request, path });
      return { ...info, path };
    },
    async exportSkill(id) {
      const path = await dialog.save({ defaultPath: `${id}.zip`, filters: [{ name: "Skill", extensions: ["zip"] }] });
      if (!path) return null;
      await invoke("skills_export_one", { id, path });
      return path;
    },
    skillsPrompt: (req) => invoke("skills_prompt", { request: requestJson(req) }),
    async savePrompt(req) {
      const request = requestJson(req);
      const suggested = (await invoke<PromptExport>("skills_prompt", { request })).fileName;
      const path = await dialog.save({ defaultPath: suggested, filters: [{ name: "Markdown", extensions: ["md"] }] });
      if (!path) return null;
      const info = await invoke<PromptExport>("skills_save_prompt", { request, path });
      return { ...info, path };
    },
    sendRequest: (req) => invoke("send_design_request", { request: JSON.parse(requestJson(req)) }),
  };
}

// --- browser -------------------------------------------------------------------------

interface StoredDesign {
  meta: DesignMeta;
  versions: VersionFile[];
  presets: Preset[];
  pending: (Pending & { design: unknown })[];
  thumbnail?: string;
}
interface Store {
  designs: Record<string, StoredDesign>;
  collections: Collection[];
}

async function browserApi(): Promise<Api> {
  const wasm = await import("./wasm/pd_wasm.js");
  await wasm.default();
  const { EXAMPLES } = await import("./examples");
  const KEY = "pd-library-v1";
  const load = (): Store => {
    try {
      return JSON.parse(localStorage.getItem(KEY) || "") as Store;
    } catch {
      return { designs: {}, collections: [] };
    }
  };
  const save = (s: Store) => {
    try {
      localStorage.setItem(KEY, JSON.stringify(s));
    } catch {
      // storage full or blocked: keep working in memory for this session
    }
  };
  let store = load();
  const get = (id: string) => {
    const d = store.designs[id];
    if (!d) throw new Error(`no design ${id}`);
    return d;
  };
  const commit = () => save(store);
  const summary = (d: StoredDesign): Summary => ({ ...d.meta, versionCount: d.versions.length, presetCount: d.presets.length, pendingCount: d.pending.length, thumbnail: d.thumbnail ?? null });
  const addVersion = (id: string, design: unknown, params: Params, author: string, message: string, rejected: boolean): VersionInfo => {
    const d = get(id);
    const num = Math.max(0, ...d.versions.map((v) => v.num)) + 1;
    const created = Date.now();
    const v: VersionFile = { file: `v${String(num).padStart(4, "0")}-${created}.json`, num, created, author, message, params, rejected, design };
    d.versions.unshift(v);
    if (!rejected) {
      d.meta.currentVersion = num;
      d.meta.modified = created;
    }
    commit();
    const { design: _d, ...info } = v;
    return info;
  };
  const create = (designText: string, params: Params, source: string, message: string, isNew = false): string => {
    const design = JSON.parse(designText);
    const m = design?.meta ?? {};
    const name = m.name || "Untitled design";
    const id = `${slug(name)}-${Date.now().toString(16)}${Math.floor(Math.random() * 256).toString(16)}`;
    const now = Date.now();
    store.designs[id] = {
      meta: { id, name, description: m.description ?? "", category: m.category ?? "", tags: m.tags ?? [], favorite: false, collections: [], created: now, modified: now, opened: null, source, isNew, trashed: null, currentVersion: 0, info: {} },
      versions: [],
      presets: [],
      pending: [],
    };
    addVersion(id, design, params, source === "claude" ? "claude" : "you", message, false);
    return id;
  };
  const download = (bytes: Uint8Array, name: string) => {
    const url = URL.createObjectURL(new Blob([bytes.slice().buffer]));
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  const SKEY = "pd-settings-v1";
  // Test hook: what the MCP server does on the desktop (a design "from Claude").
  const inboxListeners: ((e: InboxEvent[]) => void)[] = [];
  (window as unknown as { __pdClaude?: unknown }).__pdClaude = (kind: "new" | "update", designText: string, message: string, designId?: string) => {
    let ev: InboxEvent;
    const name = JSON.parse(designText)?.meta?.name ?? "design";
    if (kind === "update" && designId && store.designs[designId]) {
      const created = Date.now();
      store.designs[designId].pending.push({ file: `${created}.json`, created, message, design: JSON.parse(designText) });
      commit();
      ev = { kind: "update", id: designId, name, message };
    } else {
      const id = create(designText, {}, "claude", message || "Created by Claude", true);
      ev = { kind: "new", id, name, message };
    }
    inboxListeners.forEach((l) => l([ev]));
  };
  return {
    kind: "browser",
    evaluate: async (design, params, opts) => JSON.parse(wasm.evaluate(design, JSON.stringify(params), JSON.stringify(opts))),
    checkAll: async (design, params, opts) => JSON.parse(wasm.check_all(design, JSON.stringify(params), JSON.stringify(opts))),
    async exportFile(design, params, opts, format, part, baseName) {
      download(wasm.exportFile(design, JSON.stringify(params), JSON.stringify(opts), format, part), `${baseName}.${format}`);
      return `${baseName}.${format}`;
    },
    async getSettings() {
      try {
        return { ...JSON.parse(localStorage.getItem(SKEY) || "{}"), defaultLibraryPath: "Browser storage" };
      } catch {
        return { defaultLibraryPath: "Browser storage" };
      }
    },
    async saveSettings(s) {
      try {
        localStorage.setItem(SKEY, JSON.stringify({ ...s, defaultLibraryPath: undefined }));
      } catch {
        // ignore
      }
    },
    async pickFolder() {
      return "Browser storage";
    },
    mcpServerPath: async () => null,
    setCurrent: async (state) => {
      (window as unknown as { __pdCurrent?: CurrentState }).__pdCurrent = state;
    },
    async onInbox(cb) {
      inboxListeners.push(cb);
      return () => inboxListeners.splice(inboxListeners.indexOf(cb), 1);
    },
    onCloseRequested: async () => () => {},
    list: async () => Object.values(store.designs).map(summary).sort((a, b) => b.modified - a.modified),
    async detail(id) {
      const d = get(id);
      return { meta: d.meta, versions: d.versions.map(({ design: _d, ...v }) => v), presets: d.presets, pending: d.pending.map(({ design: _d, ...p }) => p) };
    },
    async readVersion(id, file) {
      const d = get(id);
      const v = file ? d.versions.find((x) => x.file === file) : d.versions.find((x) => x.num === d.meta.currentVersion && !x.rejected) ?? d.versions.find((x) => !x.rejected);
      if (!v) throw new Error("version not found");
      return v;
    },
    create: async (design, params, source, message) => create(design, params, source, message),
    saveVersion: async (id, design, params, author, message, rejected = false) => addVersion(id, JSON.parse(design), params, author, message, rejected),
    async updateMeta(id, patch) {
      const d = get(id);
      d.meta = { ...d.meta, ...patch, id };
      commit();
      return d.meta;
    },
    async setPresets(id, presets) {
      get(id).presets = presets;
      commit();
    },
    async setThumbnail(id, png) {
      get(id).thumbnail = png;
      commit();
    },
    async deleteForever(id) {
      delete store.designs[id];
      commit();
    },
    async duplicate(id, file, name) {
      const src = get(id);
      const v = file ? src.versions.find((x) => x.file === file)! : src.versions.find((x) => x.num === src.meta.currentVersion)!;
      const design = JSON.parse(JSON.stringify(v.design)) as { meta?: { name?: string } };
      design.meta = { ...(design.meta ?? {}), name };
      const nid = create(JSON.stringify(design), v.params, "you", `Copied from “${src.meta.name}”`);
      const n = get(nid);
      n.meta = { ...n.meta, collections: [...src.meta.collections], tags: [...src.meta.tags], category: src.meta.category, info: src.meta.info };
      n.thumbnail = src.thumbnail;
      commit();
      return nid;
    },
    async combine(id, other) {
      if (id === other) throw new Error("pick two different designs");
      const a = get(id);
      const b = get(other);
      const from = b.meta.modified > a.meta.modified ? b : a;
      const all = [a, b]
        .flatMap((d) =>
          d.versions.map((v) => ({
            ...v,
            message: d === b && b.meta.name !== a.meta.name ? `${v.message || "Saved"} (from “${b.meta.name}”)` : v.message,
            current: d === from && v.num === from.meta.currentVersion && !v.rejected,
          })),
        )
        .sort((x, y) => x.created - y.created);
      let current = 0;
      const joined: VersionFile[] = all.map(({ current: isCurrent, ...v }, i) => {
        const num = i + 1;
        if (isCurrent) current = num;
        return { ...v, num, file: `v${String(num).padStart(4, "0")}-${v.created}.json` };
      });
      a.versions = joined.reverse();
      a.meta.currentVersion = current || (a.versions.find((v) => !v.rejected)?.num ?? 0);
      a.meta.modified = Math.max(a.meta.modified, b.meta.modified);
      a.meta.favorite = a.meta.favorite || b.meta.favorite;
      a.meta.isNew = false;
      a.meta.collections = [...new Set([...a.meta.collections, ...b.meta.collections])];
      a.meta.tags = [...new Set([...a.meta.tags, ...b.meta.tags])];
      a.presets = [...a.presets, ...b.presets.filter((p) => !a.presets.some((q) => q.name === p.name))];
      a.pending = [...a.pending, ...b.pending];
      if (from === b) {
        a.thumbnail = b.thumbnail;
        a.meta.info = b.meta.info;
      }
      b.meta.trashed = Date.now();
      commit();
      return a.meta;
    },
    async readPending(id, file) {
      const p = get(id).pending.find((x) => x.file === file);
      if (!p) throw new Error("that update is gone");
      return p;
    },
    async removePending(id, file) {
      const d = get(id);
      d.pending = d.pending.filter((x) => x.file !== file);
      commit();
    },
    collections: async () => store.collections ?? [],
    async setCollections(c) {
      store.collections = c;
      commit();
    },
    importFiles() {
      return new Promise((resolve) => {
        const input = document.createElement("input");
        input.type = "file";
        input.multiple = true;
        input.accept = ".json,.3mf";
        input.onchange = async () => {
          const ids: string[] = [];
          for (const f of Array.from(input.files ?? [])) {
            try {
              if (f.name.toLowerCase().endsWith(".3mf")) {
                const emb = JSON.parse(wasm.import_3mf(new Uint8Array(await f.arrayBuffer())));
                ids.push(create(JSON.stringify(emb.design), emb.params ?? {}, "import", `Imported from ${f.name}`));
              } else {
                ids.push(create(await f.text(), {}, "import", `Imported from ${f.name}`));
              }
            } catch {
              // skip files that are not designs
            }
          }
          resolve(ids);
        };
        input.click();
      });
    },
    async addExamples() {
      for (const e of EXAMPLES) create(e.text, {}, "example", "Example design");
      return EXAMPLES.length;
    },
    skillsCatalog: async () => JSON.parse(wasm.skills_catalog()),
    async exportSkillBundle(req) {
      const request = requestJson(req);
      const info = JSON.parse(wasm.skills_bundle_info(request));
      download(wasm.skills_bundle_zip(request), info.fileName);
      return { ...info, path: info.fileName };
    },
    async exportSkill(id) {
      download(wasm.skill_zip(id), `${id}.zip`);
      return `${id}.zip`;
    },
    skillsPrompt: async (req) => JSON.parse(wasm.skills_prompt(requestJson(req), undefined)),
    async savePrompt(req) {
      const info: PromptExport = JSON.parse(wasm.skills_prompt(requestJson(req), undefined));
      download(new TextEncoder().encode(info.prompt), info.fileName);
      return { ...info, path: info.fileName };
    },
    async sendRequest() {
      throw new Error("Sending a request to Claude needs the desktop app.");
    },
  };
}

export function loadApi(): Promise<Api> {
  return isTauri ? desktopApi() : browserApi();
}

// The engine behind the UI. In the desktop app it is the native Rust core
// (Tauri commands); in a plain browser it is the same core compiled to
// WebAssembly. Both return identical JSON.

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

export interface OpenedDesign {
  text: string;
  params: Params;
  fileName: string;
}

export interface Engine {
  kind: "desktop" | "browser";
  evaluate(design: string, params: Params): Promise<Evaluation>;
  checkAll(design: string, params: Params): Promise<Report>;
  /** Builds and saves a print file; resolves to where it went (or null if cancelled). */
  saveExport(design: string, params: Params, format: "stl" | "3mf", baseName: string): Promise<string | null>;
  /** Asks the user for a .pd.json or .3mf file. */
  openFile(): Promise<OpenedDesign | null>;
  /** Called with designs that LLMs send through the MCP server (desktop only). */
  onInbox(cb: (d: OpenedDesign) => void): Promise<() => void>;
  /** Path of the bundled `pd` MCP server, if installed next to the app (desktop only). */
  mcpServerPath(): Promise<string | null>;
}

const isTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function desktopEngine(): Promise<Engine> {
  const { invoke } = await import("@tauri-apps/api/core");
  const { listen } = await import("@tauri-apps/api/event");
  const dialog = await import("@tauri-apps/plugin-dialog");
  return {
    kind: "desktop",
    evaluate: (design, params) => invoke<Evaluation>("evaluate", { design, params: JSON.stringify(params) }),
    checkAll: (design, params) => invoke<Report>("check_all", { design, params: JSON.stringify(params) }),
    async saveExport(design, params, format, baseName) {
      const path = await dialog.save({ defaultPath: `${baseName}.${format}`, filters: [{ name: format.toUpperCase(), extensions: [format] }] });
      if (!path) return null;
      await invoke("export_to_file", { design, params: JSON.stringify(params), format, path });
      return path;
    },
    async openFile() {
      const path = await dialog.open({ multiple: false, filters: [{ name: "Designs", extensions: ["json", "3mf"] }] });
      if (!path || Array.isArray(path)) return null;
      return invoke<OpenedDesign>("open_design_file", { path });
    },
    mcpServerPath: () => invoke<string | null>("mcp_server_path"),
    async onInbox(cb) {
      return listen<OpenedDesign>("inbox-design", (e) => cb(e.payload));
    },
  };
}

async function browserEngine(): Promise<Engine> {
  const wasm = await import("./wasm/pd_wasm.js");
  await wasm.default();
  const download = (bytes: Uint8Array, name: string) => {
    const url = URL.createObjectURL(new Blob([bytes.slice().buffer]));
    const a = document.createElement("a");
    a.href = url;
    a.download = name;
    a.click();
    setTimeout(() => URL.revokeObjectURL(url), 1000);
  };
  return {
    kind: "browser",
    evaluate: async (design, params) => JSON.parse(wasm.evaluate(design, JSON.stringify(params))),
    checkAll: async (design, params) => JSON.parse(wasm.check_all(design, JSON.stringify(params))),
    async saveExport(design, params, format, baseName) {
      const bytes = wasm.exportFile(design, JSON.stringify(params), format, undefined);
      download(bytes, `${baseName}.${format}`);
      return `${baseName}.${format}`;
    },
    openFile() {
      return new Promise((resolve) => {
        const input = document.createElement("input");
        input.type = "file";
        input.accept = ".json,.3mf";
        input.onchange = async () => {
          const f = input.files?.[0];
          if (!f) return resolve(null);
          if (f.name.toLowerCase().endsWith(".3mf")) {
            const embedded = JSON.parse(wasm.import_3mf(new Uint8Array(await f.arrayBuffer())));
            resolve({ text: JSON.stringify(embedded.design, null, 2), params: embedded.params ?? {}, fileName: f.name });
          } else {
            resolve({ text: await f.text(), params: {}, fileName: f.name });
          }
        };
        input.click();
      });
    },
    async onInbox() {
      return () => {};
    },
    async mcpServerPath() {
      return null;
    },
  };
}

export function loadEngine(): Promise<Engine> {
  return isTauri ? desktopEngine() : browserEngine();
}

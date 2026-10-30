// App settings, printer profiles and unit conversion.

export type ThemePref = "system" | "light" | "dark";
export type Units = "mm" | "in";

export interface PrinterSetting {
  id: string;
  name: string;
  bed: [number, number, number];
  nozzle: number;
}

export interface Settings {
  libraryPath?: string;
  /** Suggested folder (provided by the backend, never saved). */
  defaultLibraryPath?: string;
  theme: ThemePref;
  accent: string;
  textSize: "normal" | "large";
  units: Units;
  printer: PrinterSetting;
}

export const PRINTERS: PrinterSetting[] = [
  { id: "bambu-a1", name: "Bambu Lab A1", bed: [256, 256, 256], nozzle: 0.4 },
  { id: "bambu-a1-mini", name: "Bambu Lab A1 mini", bed: [180, 180, 180], nozzle: 0.4 },
  { id: "bambu-p1s", name: "Bambu Lab P1S", bed: [256, 256, 256], nozzle: 0.4 },
  { id: "bambu-x1c", name: "Bambu Lab X1 Carbon", bed: [256, 256, 256], nozzle: 0.4 },
  { id: "prusa-mk4s", name: "Prusa MK4S", bed: [250, 210, 220], nozzle: 0.4 },
  { id: "prusa-core-one", name: "Prusa CORE One", bed: [250, 220, 270], nozzle: 0.4 },
  { id: "creality-ender3-v3", name: "Creality Ender-3 V3", bed: [220, 220, 250], nozzle: 0.4 },
  { id: "creality-k1", name: "Creality K1", bed: [220, 220, 250], nozzle: 0.4 },
];

export const ACCENTS: { hex: string; name: string }[] = [
  { hex: "#2563eb", name: "Blue" },
  { hex: "#0e7490", name: "Teal" },
  { hex: "#15803d", name: "Green" },
  { hex: "#c2410c", name: "Orange" },
  { hex: "#be123c", name: "Rose" },
  { hex: "#7e22ce", name: "Purple" },
];

export const DEFAULT_SETTINGS: Settings = {
  theme: "system",
  accent: "#2563eb",
  textSize: "normal",
  units: "mm",
  printer: PRINTERS[2],
};

export function withDefaults(s: Partial<Settings>): Settings {
  return { ...DEFAULT_SETTINGS, ...s, printer: s.printer?.bed ? s.printer : DEFAULT_SETTINGS.printer };
}

export function evalOpts(s: Settings) {
  return { printer: { bed: s.printer.bed, nozzle: s.printer.nozzle } };
}

// --- units ---------------------------------------------------------------------

export function fmtNum(v: number, digits = 2): string {
  if (!Number.isFinite(v)) return "–";
  const r = Math.round(v * 10 ** digits) / 10 ** digits;
  return String(r);
}

/** A length in mm, shown in the user's units. */
export function fmtLen(mm: number, units: Units, digits?: number): string {
  return units === "in" ? `${fmtNum(mm / 25.4, digits ?? 3)} in` : `${fmtNum(mm, digits ?? 1)} mm`;
}

export function fmtSize(s: number[] | undefined, units: Units): string {
  if (!s || s.length < 3) return "";
  const k = units === "in" ? 25.4 : 1;
  const d = units === "in" ? 2 : 1;
  return `${s.map((v) => fmtNum(v / k, d)).join(" × ")} ${units === "in" ? "in" : "mm"}`;
}

export function timeAgo(ms: number): string {
  const s = Math.max(0, (Date.now() - ms) / 1000);
  if (s < 60) return "just now";
  const m = s / 60;
  if (m < 60) return `${Math.floor(m)} min ago`;
  const h = m / 60;
  if (h < 24) return `${Math.floor(h)} h ago`;
  const d = h / 24;
  if (d < 2) return "yesterday";
  if (d < 7) return `${Math.floor(d)} days ago`;
  if (d < 14) return "last week";
  if (d < 31) return `${Math.floor(d / 7)} weeks ago`;
  return new Date(ms).toLocaleDateString();
}

export function slug(s: string): string {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "").slice(0, 48) || "design";
}

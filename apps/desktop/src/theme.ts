// Light/dark palettes and the accent colour, applied as CSS variables.
import type { Settings } from "./settings";

export interface Palette {
  dark: boolean;
  bg: string;
  panel: string;
  panel2: string;
  raised: string;
  border: string;
  text: string;
  muted: string;
  faint: string;
  viewer: string;
  plate: string;
  grid: string;
  gridMajor: string;
  warn: string;
  ok: string;
  danger: string;
  accent: string;
  accentText: string;
  accentSoft: string;
  accentBorder: string;
}

const DARK = {
  bg: "#0f1115", panel: "#16191e", panel2: "#1c2027", raised: "#262b34", border: "#2a2f38",
  text: "#e8ebef", muted: "#a1a9b4", faint: "#8a93a0", viewer: "#23272e", plate: "#2b3038",
  grid: "#353b45", gridMajor: "#4a525e", warn: "#f0a43a", ok: "#3fbf6a", danger: "#f06a6a",
};
const LIGHT = {
  bg: "#f2f3f5", panel: "#ffffff", panel2: "#f6f7f9", raised: "#eceef1", border: "#dde1e6",
  text: "#14171c", muted: "#545d69", faint: "#636c78", viewer: "#e4e7eb", plate: "#f4f5f7",
  grid: "#d3d8de", gridMajor: "#b6bdc6", warn: "#a35a00", ok: "#18803d", danger: "#c62828",
};

export function mix(a: string, b: string, t: number): string {
  const p = (h: string) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));
  const x = p(a.length === 4 ? "#" + [...a.slice(1)].map((c) => c + c).join("") : a);
  const y = p(b);
  return "#" + x.map((v, i) => Math.round(v + (y[i] - v) * t).toString(16).padStart(2, "0")).join("");
}

export function systemDark(): boolean {
  return typeof window !== "undefined" && window.matchMedia?.("(prefers-color-scheme: dark)").matches;
}

export function palette(s: Settings): Palette {
  const dark = s.theme === "dark" || (s.theme === "system" && systemDark());
  const base = dark ? DARK : LIGHT;
  const accent = /^#[0-9a-fA-F]{6}$/.test(s.accent) ? s.accent : "#2563eb";
  return {
    dark,
    ...base,
    accent,
    accentText: dark ? mix(accent, "#ffffff", 0.4) : mix(accent, "#000000", 0.12),
    accentSoft: mix(accent, base.panel, dark ? 0.84 : 0.9),
    accentBorder: mix(accent, base.panel, dark ? 0.55 : 0.6),
  };
}

export function applyTheme(s: Settings): Palette {
  const p = palette(s);
  const root = document.documentElement;
  root.dataset.theme = p.dark ? "dark" : "light";
  root.style.colorScheme = p.dark ? "dark" : "light";
  root.style.fontSize = s.textSize === "large" ? "17px" : "15px";
  for (const [k, v] of Object.entries(p)) {
    if (typeof v === "string") root.style.setProperty("--" + k.replace(/[A-Z]/g, (m) => "-" + m.toLowerCase()), v);
  }
  return p;
}

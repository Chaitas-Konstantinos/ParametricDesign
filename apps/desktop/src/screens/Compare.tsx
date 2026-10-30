import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../App";
import type { Evaluation, Params, ParamValue } from "../api";
import { Icon } from "../components/Icon";
import { Segmented } from "../components/ui";
import { Viewer, type CameraLink, type ViewerHandle } from "../components/Viewer";
import { evalOpts, fmtNum, fmtSize, timeAgo } from "../settings";

export interface PendingUpdate {
  file: string;
  created: number;
  message: string;
  design: unknown;
}

interface Props {
  name: string;
  version: number;
  current: Evaluation;
  values: Params;
  pending: PendingUpdate;
  onAccept(designText: string, params: Params): void;
  onSaveAsNew(designText: string, params: Params): void;
  onReject(designText: string, params: Params): void;
  onClose(): void;
}

function show(v: ParamValue | undefined, unit?: string) {
  if (v === undefined) return "—";
  if (typeof v === "number") return `${fmtNum(v, 3)}${unit ? ` ${unit}` : ""}`;
  return String(v);
}

export function Compare({ name, version, current, values, pending, onAccept, onSaveAsNew, onReject, onClose }: Props) {
  const { api, settings, colors } = useApp();
  const [mode, setMode] = useState<"side" | "overlay">("side");
  const [next, setNext] = useState<Evaluation | null>(null);
  const [error, setError] = useState<string | null>(null);
  const designText = useMemo(() => JSON.stringify(pending.design, null, 2), [pending]);
  const link = useRef<CameraLink>({ pos: null, target: null, rev: 0, src: "" }).current;
  const h1 = useRef<ViewerHandle | null>(null);
  const h2 = useRef<ViewerHandle | null>(null);

  useEffect(() => {
    api.evaluate(designText, values, evalOpts(settings)).then(setNext, (e) => setError(String(e)));
  }, [designText]);

  const nextParams = useMemo<Params>(() => {
    if (!next) return {};
    return Object.fromEntries(next.report.params.map((p) => [p.name, p.value]));
  }, [next]);

  const changes = useMemo(() => {
    if (!next) return [];
    const before = new Map(current.report.params.map((p) => [p.name, p]));
    const after = new Map(next.report.params.map((p) => [p.name, p]));
    const out: { tag: "NEW" | "CHANGED" | "REMOVED"; label: string; from: string; to: string }[] = [];
    for (const [n, p] of after) {
      const b = before.get(n);
      if (!b) out.push({ tag: "NEW", label: p.label ?? n, from: "—", to: show(p.default, p.unit) });
      else if (b.default !== p.default || b.min !== p.min || b.max !== p.max)
        out.push({ tag: "CHANGED", label: p.label ?? n, from: show(b.default, b.unit) + (b.min !== p.min || b.max !== p.max ? ` (${b.min}–${b.max})` : ""), to: show(p.default, p.unit) + (b.min !== p.min || b.max !== p.max ? ` (${p.min}–${p.max})` : "") });
    }
    for (const [n, p] of before) if (!after.has(n)) out.push({ tag: "REMOVED", label: p.label ?? n, from: show(p.default, p.unit), to: "—" });
    return out;
  }, [next, current]);

  const parts = useMemo(() => {
    if (!next) return [];
    const ids = new Set([...current.report.parts.map((p) => p.id), ...next.report.parts.map((p) => p.id)]);
    return [...ids].map((id) => {
      const a = current.report.parts.find((p) => p.id === id);
      const b = next.report.parts.find((p) => p.id === id);
      const same = a && b && a.size.every((v, i) => Math.abs(v - b.size[i]) < 0.05) && Math.abs(a.volume_mm3 - b.volume_mm3) < 1;
      const sameSize = a && b && a.size.every((v, i) => Math.abs(v - b.size[i]) < 0.05);
      const text = !a ? `new part, ${fmtSize(b!.size, settings.units)}` : !b ? "removed" : same ? "unchanged" : sameSize ? `same size, shape changed (${b.volume_mm3 > a.volume_mm3 ? "+" : "−"}${fmtNum(Math.abs(b.volume_mm3 - a.volume_mm3) / 1000, 1)} cm³)` : `${fmtSize(a.size, settings.units)} → ${fmtSize(b.size, settings.units)}`;
      return { id, name: (b ?? a)!.name ?? id, text };
    });
  }, [next, current]);

  const kept = (next?.report.params ?? []).filter((p) => p.name in values && (p.type === "number" || p.type === "integer")).map((p) => `${(p.label ?? p.name).toLowerCase()} ${show(values[p.name], p.unit)}`);
  const ok = next?.report.ok ?? false;
  const common = { display: "edges" as const, ortho: false, plate: true, bed: settings.printer.bed, colors, units: settings.units, showCube: false };

  return (
    <div className="compare" role="dialog" aria-label="Compare Claude's update">
      <header className="topbar compare-head">
        <span className="claude-mark">
          <Icon name="sparkle" size={22} />
        </span>
        <div className="grow">
          <div className="strong">Claude wants to update “{name}”</div>
          <div className="muted small">
            {pending.message ? `“${pending.message}”` : "No description given"} · {timeAgo(pending.created)}
            {next && (ok ? " · builds without errors" : ` · ${next.report.errors.length} error(s)`)}
          </div>
        </div>
        <Segmented
          label="Compare mode"
          value={mode}
          onChange={setMode}
          options={[
            { value: "side", label: "Side by side" },
            { value: "overlay", label: "Overlay" },
          ]}
        />
        <button type="button" className="icon-btn" aria-label="Close compare (decide later)" title="Decide later" onClick={onClose}>
          <Icon name="close" />
        </button>
      </header>
      <div className="compare-body">
        <div className={`compare-views ${mode}`}>
          {mode === "side" ? (
            <>
              <div className="compare-pane">
                <Viewer {...common} meshes={current.meshes} frameKey="cmp" link={link} handleRef={h1} label={`Current · v${version}`} />
              </div>
              <div className="compare-pane">
                {next ? <Viewer {...common} meshes={next.meshes} frameKey="cmp" link={link} handleRef={h2} label={`Claude’s update · becomes v${version + 1}`} /> : <div className="splash">{error ?? "Building the update…"}</div>}
              </div>
            </>
          ) : (
            <div className="compare-pane">
              {next ? <Viewer {...common} meshes={next.meshes} ghost={current.meshes} frameKey="cmp-o" handleRef={h1} label="Update (solid) over current (dashed)" /> : <div className="splash">{error ?? "Building the update…"}</div>}
            </div>
          )}
        </div>
        <aside className="panel compare-side">
          <h2>What changes</h2>
          {!next && !error && <p className="muted">Building…</p>}
          {error && <div className="banner error">{error}</div>}
          {next && !ok && (
            <div className="banner error">
              The update has errors: {next.report.errors[0]?.message}
              {next.report.errors.length > 1 ? ` (+${next.report.errors.length - 1} more)` : ""}. Ask Claude to fix it, or reject it.
            </div>
          )}
          <h3 className="section-title">Parameters</h3>
          {next && changes.length === 0 && <p className="muted small">No parameter changes.</p>}
          {changes.map((c) => (
            <div key={c.tag + c.label} className="change-row">
              <span className={`tag ${c.tag.toLowerCase()}`}>{c.tag}</span>
              <span className="grow">{c.label}</span>
              <span className="muted num">{c.from}</span>
              <Icon name="chevronRight" size={15} />
              <span className="strong num">{c.to}</span>
            </div>
          ))}
          <h3 className="section-title">Parts</h3>
          {parts.map((p) => (
            <div key={p.id} className="part-change">
              <b>{p.name}</b> <span className="muted">{p.text}</span>
            </div>
          ))}
          {kept.length > 0 && <div className="banner accent small">Your slider values are kept: {kept.slice(0, 4).join(", ")}{kept.length > 4 ? ` and ${kept.length - 4} more` : ""}.</div>}
          <div className="grow" />
          <div className="compare-actions">
            <button type="button" className="btn primary big" disabled={!next} onClick={() => onAccept(designText, nextParams)}>
              Accept and save as v{version + 1}
            </button>
            <div className="row">
              <button type="button" className="btn grow" disabled={!next} onClick={() => onSaveAsNew(designText, nextParams)}>
                Save as a new design
              </button>
              <button type="button" className="btn grow" onClick={() => onReject(designText, nextParams)}>
                Reject
              </button>
            </div>
            <p className="muted small center">Rejected updates stay in Versions, marked “not accepted”.</p>
          </div>
        </aside>
      </div>
    </div>
  );
}

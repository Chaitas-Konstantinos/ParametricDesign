import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../App";
import type { Detail, Evaluation, Issue, Params, ParamValue, Preset, Report } from "../api";
import { Icon } from "../components/Icon";
import { ParamPanel } from "../components/ParamPanel";
import { MenuButton, Segmented, useUi, type MenuItem } from "../components/ui";
import { Viewer, type DisplayMode, type MeasureResult, type SectionState, type V3, type ViewerHandle } from "../components/Viewer";
import { evalOpts, fmtLen, fmtNum, fmtSize, slug, timeAgo } from "../settings";
import { renderThumbnail } from "../thumbnail";
import { Compare, type PendingUpdate } from "./Compare";

const DISPLAY: { value: DisplayMode; label: string }[] = [
  { value: "shaded", label: "Shaded" },
  { value: "edges", label: "Shaded + edges" },
  { value: "wire", label: "Wireframe" },
  { value: "xray", label: "X-ray" },
];

function sameParams(a: Params, b: Params): boolean {
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  for (const k of keys) if (a[k] !== b[k]) return false;
  return true;
}

function fmtValue(v: ParamValue | undefined): string {
  return typeof v === "number" ? fmtNum(v, 3) : String(v);
}

function describeChanges(before: Params, after: Params, report: Report | undefined): string {
  const label = (n: string) => report?.params.find((p) => p.name === n)?.label ?? n;
  const changed = Object.keys(after).filter((k) => before[k] !== after[k]);
  if (!changed.length) return "Saved";
  const parts = changed.slice(0, 3).map((k) => `${label(k)} ${fmtValue(before[k])} → ${fmtValue(after[k])}`);
  return parts.join(", ") + (changed.length > 3 ? ` (+${changed.length - 3} more)` : "");
}

function fixItMessage(name: string, issues: Issue[]): string {
  const lines = issues.map((i) => `- ${i.severity.toUpperCase()} at ${i.path || "(design)"}: ${i.message}${i.when ? ` (when ${i.when})` : ""}${i.hint ? `\n  fix: ${i.hint}` : ""}`);
  return `ParametricDesign reports these problems with “${name}”. Please fix all of them and send the corrected design (open_in_app mode "update"):\n\n${lines.join("\n")}`;
}

export function EditorScreen({ id, review, version: openVersion }: { id: string; review?: string; version?: string }) {
  const { api, settings, colors, go, leaveGuard, lastInbox, bumpLibrary } = useApp();
  const ui = useUi();
  const [detail, setDetail] = useState<Detail | null>(null);
  const [designText, setDesignText] = useState("");
  const [savedText, setSavedText] = useState("");
  const [values, setValues] = useState<Params>({});
  const [saved, setSaved] = useState<Params>({});
  const [restoredFrom, setRestoredFrom] = useState<number | null>(null);
  const [evaluation, setEvaluation] = useState<Evaluation | null>(null);
  const [lastGood, setLastGood] = useState<Evaluation | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [advanced, setAdvanced] = useState(false);
  const [display, setDisplay] = useState<DisplayMode>("edges");
  const [ortho, setOrtho] = useState(false);
  const [plate, setPlate] = useState(true);
  const [exploded, setExploded] = useState(false);
  const [measuring, setMeasuring] = useState(false);
  const [measure, setMeasure] = useState<MeasureResult | null>(null);
  const [section, setSection] = useState<SectionState>({ on: false, axis: "y", at: 0 });
  const [bounds, setBounds] = useState<{ min: V3; max: V3 } | null>(null);
  const [hidden, setHidden] = useState<Set<string>>(new Set());
  const [compare, setCompare] = useState<PendingUpdate | null>(null);
  const [checkAll, setCheckAll] = useState<Report | "busy" | null>(null);
  const [versionsOpen, setVersionsOpen] = useState(false);
  // after loading a version: turn its saved values into a full set (keepSaved: restoring, so the old save stays the reference)
  const normalize = useRef<{ params: Params; keepSaved: boolean } | null>(null);
  // a version picked in the library: shown once the current one has loaded
  const showVersion = useRef<string | null>(openVersion ?? null);
  const [tick, setTick] = useState(0);
  const needsThumb = useRef(false);
  const history = useRef<{ past: Params[]; future: Params[]; last: { key: string; t: number } | null }>({ past: [], future: [], last: null });
  const viewer = useRef<ViewerHandle | null>(null);
  const [frameKey] = useState(() => `${id}-${Date.now()}`);

  const name = detail?.meta.name ?? "…";
  const version = detail?.meta.currentVersion ?? 0;
  const report = evaluation?.report;
  const dirty = designText !== savedText || !sameParams(values, saved);

  const loadVersion = useCallback(
    async (file?: string, keepSaved = false) => {
      const v = await api.readVersion(id, file);
      const text = JSON.stringify(v.design, null, 2);
      normalize.current = { params: v.params ?? {}, keepSaved };
      setDesignText(text);
      setTick((t) => t + 1);
      return { text, v };
    },
    [api, id],
  );

  // initial load
  useEffect(() => {
    (async () => {
      try {
        const d = await api.detail(id);
        setDetail(d);
        const pick = d.versions.find((v) => v.file === showVersion.current);
        if (!pick || (pick.num === d.meta.currentVersion && !pick.rejected)) showVersion.current = null;
        const { text } = await loadVersion();
        setSavedText(text);
        void api.updateMeta(id, { opened: Date.now(), isNew: false });
      } catch (e) {
        setFatal(String(e));
      }
    })();
  }, [id]);

  // rebuild on every change
  useEffect(() => {
    if (!designText) return;
    let cancelled = false;
    const t = setTimeout(async () => {
      setBusy(true);
      try {
        const params = normalize.current?.params ?? values;
        const ev = await api.evaluate(designText, params, evalOpts(settings));
        if (cancelled) return;
        setEvaluation(ev);
        setFatal(null);
        if (normalize.current) {
          const full = Object.fromEntries(ev.report.params.map((p) => [p.name, p.value]));
          const keep = normalize.current.keepSaved;
          normalize.current = null;
          setValues(full);
          if (!keep) setSaved(full);
          if (!keep && showVersion.current) {
            const file = showVersion.current;
            showVersion.current = null;
            const num = Number(/^v(\d+)-/.exec(file)?.[1] ?? 0);
            void restore(file, num).then(() => ui.toast(`Showing v${num}. Save (Ctrl+S) to make it the current version.`, { kind: "info" }));
          }
        }
        if (ev.meshes.length) {
          setLastGood(ev);
          if (needsThumb.current) {
            needsThumb.current = false;
            const th = renderThumbnail(ev.meshes);
            if (th) {
              await api.setThumbnail(id, th.png);
              await api.updateMeta(id, { info: th.info });
            }
          }
        }
      } catch (e) {
        if (!cancelled) setFatal(String(e));
      } finally {
        if (!cancelled) setBusy(false);
      }
    }, 60);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [designText, values, settings.printer, tick]);

  // tell Claude (MCP) what is open
  useEffect(() => {
    if (!detail || !designText) return;
    const t = setTimeout(() => {
      let design: unknown = null;
      try {
        design = JSON.parse(designText);
      } catch {
        design = designText;
      }
      void api.setCurrent({ open: true, designId: id, name, version, unsaved: dirty, params: values, design });
    }, 400);
    return () => clearTimeout(t);
  }, [detail, designText, values, dirty]);

  const reloadDetail = async () => {
    const d = await api.detail(id);
    setDetail(d);
    return d;
  };

  const openPending = async (file?: string) => {
    const d = detail ?? (await reloadDetail());
    const p = file ? d.pending.find((x) => x.file === file) : d.pending[d.pending.length - 1];
    if (!p) return;
    const full = await api.readPending(id, p.file);
    setCompare({ file: p.file, created: p.created, message: p.message, design: full.design });
  };

  // opened from a "Review" action, or an update arrives while open
  useEffect(() => {
    if (review && detail?.pending.length && evaluation && !compare) void openPending();
  }, [review, detail, !!evaluation]);
  useEffect(() => {
    const ev = lastInbox.events.find((e) => e.kind === "update" && e.id === id);
    if (!ev) return;
    (async () => {
      const d = await reloadDetail();
      const p = d.pending[d.pending.length - 1];
      if (p && evaluation) {
        const full = await api.readPending(id, p.file);
        setCompare({ file: p.file, created: p.created, message: p.message, design: full.design });
      }
    })();
  }, [lastInbox.rev]);

  // --- editing ----------------------------------------------------------------------
  const setParam = (n: string, v: ParamValue) => {
    const h = history.current;
    const now = Date.now();
    if (!h.last || h.last.key !== n || now - h.last.t > 600) h.past.push(values);
    h.last = { key: n, t: now };
    h.future = [];
    setValues((old) => ({ ...old, [n]: v }));
  };
  const applyParams = (p: Params) => {
    history.current.past.push(values);
    history.current.future = [];
    history.current.last = null;
    setValues((old) => ({ ...old, ...p }));
  };
  const undo = () => {
    const h = history.current;
    const prev = h.past.pop();
    if (!prev) return;
    h.future.push(values);
    h.last = null;
    setValues(prev);
  };
  const redo = () => {
    const h = history.current;
    const n = h.future.pop();
    if (!n) return;
    h.past.push(values);
    h.last = null;
    setValues(n);
  };

  const save = async (): Promise<boolean> => {
    if (!dirty) return true;
    try {
      const message = restoredFrom ? `Restored v${restoredFrom}` : describeChanges(saved, values, report);
      const info = await api.saveVersion(id, designText, values, "you", message);
      const th = lastGood ? renderThumbnail(lastGood.meshes) : null;
      if (th) {
        await api.setThumbnail(id, th.png);
        await api.updateMeta(id, { info: th.info });
      }
      setSaved(values);
      setSavedText(designText);
      setRestoredFrom(null);
      await reloadDetail();
      bumpLibrary();
      ui.toast(`Saved as v${info.num}`, { kind: "ok" });
      return true;
    } catch (e) {
      ui.toast(`Could not save: ${e}`, { kind: "error" });
      return false;
    }
  };

  // leaving with unsaved changes
  useEffect(() => {
    leaveGuard.current = async () => {
      if (!dirty) return true;
      const r = await ui.dialog({
        title: `Save changes to “${name}”?`,
        body: <p>Your changes become version v{(detail?.versions[0]?.num ?? version) + 1}. If you don't save, they are lost.</p>,
        actions: [
          { label: "Cancel", value: "cancel" },
          { label: "Don't save", value: "discard" },
          { label: "Save", value: "save", kind: "primary" },
        ],
      });
      if (r?.action === "save") return save();
      return r?.action === "discard";
    };
    return () => {
      leaveGuard.current = null;
    };
  });

  const presets = detail?.presets ?? [];
  const activePreset = presets.find((p) => Object.entries(p.params).every(([k, v]) => values[k] === v));
  const setPresets = async (next: Preset[]) => {
    await api.setPresets(id, next);
    await reloadDetail();
  };
  const presetMenu: MenuItem[] = [
    {
      label: "Save current values as a new preset…",
      icon: "plus",
      run: async () => {
        const n = await ui.prompt("New preset", "Preset name (e.g. “microSD cards”)", "", "Save preset");
        if (!n) return;
        await setPresets([...presets.filter((p) => p.name !== n), { name: n, params: values }]);
        ui.toast(`Preset “${n}” saved`, { kind: "ok" });
      },
    },
    ...(activePreset
      ? [
          {
            label: `Rename “${activePreset.name}”…`,
            icon: "edit",
            run: async () => {
              const n = await ui.prompt("Rename preset", "Name", activePreset.name);
              if (n) await setPresets(presets.map((p) => (p === activePreset ? { ...p, name: n } : p)));
            },
          },
          {
            label: `Delete “${activePreset.name}”`,
            icon: "trash",
            danger: true,
            run: async () => {
              if (await ui.confirm(`Delete preset “${activePreset.name}”?`, "The design itself is not changed.", "Delete", true)) await setPresets(presets.filter((p) => p !== activePreset));
            },
          },
        ]
      : presets.length
        ? [
            {
              label: "Overwrite a preset with these values…",
              icon: "edit",
              run: async () => {
                const n = await ui.prompt("Overwrite preset", `Preset name (${presets.map((p) => p.name).join(", ")})`, presets[0].name, "Overwrite");
                if (n && presets.some((p) => p.name === n)) await setPresets(presets.map((p) => (p.name === n ? { ...p, params: values } : p)));
              },
            },
          ]
        : []),
  ];

  const opts = evalOpts(settings);
  const baseName = slug(name) + (activePreset ? `-${slug(activePreset.name)}` : "");
  const doExport = async (format: "3mf" | "stl", part?: string) => {
    try {
      const where = await api.exportFile(designText, values, opts, format, part, part ? `${baseName}-${slug(part)}` : baseName);
      if (where) ui.toast(`Exported ${where}`, { kind: "ok" });
    } catch (e) {
      ui.toast(`Export failed: ${e}`, { kind: "error" });
    }
  };
  const exportItems: MenuItem[] = [
    { label: "3MF – all parts, colours, plate", icon: "download", hint: "Ctrl+E", run: () => void doExport("3mf") },
    { label: "STL – all parts", icon: "download", run: () => void doExport("stl") },
    ...((report?.parts.length ?? 0) > 1 ? report!.parts.map((p) => ({ label: `STL – ${p.name ?? p.id} only`, icon: "download", run: () => void doExport("stl", p.id) })) : []),
  ];

  const issues: Issue[] = [...(report?.errors ?? []), ...(report?.warnings ?? []), ...(checkAll && checkAll !== "busy" ? checkAll.errors.filter((e) => e.when) : [])];
  const copyFixIt = async () => {
    await navigator.clipboard.writeText(fixItMessage(name, issues));
    ui.toast("Copied. Paste it into your Claude chat.", { kind: "ok" });
  };

  const restore = async (file: string, num: number) => {
    setVersionsOpen(false);
    history.current = { past: [], future: [], last: null };
    await loadVersion(file, true);
    setRestoredFrom(num);
  };

  // --- keyboard ------------------------------------------------------------------------
  const keys = useRef<(e: KeyboardEvent) => void>(() => {});
  keys.current = (e: KeyboardEvent) => {
    const mod = e.ctrlKey || e.metaKey;
    const typing = (e.target as HTMLElement)?.closest?.("input, textarea, select, [contenteditable]");
    if (document.querySelector(".modal") || compare) return;
    if (mod && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void save();
    } else if (mod && e.key.toLowerCase() === "e") {
      e.preventDefault();
      void doExport("3mf");
    } else if (mod && (e.key.toLowerCase() === "y" || (e.shiftKey && e.key.toLowerCase() === "z"))) {
      e.preventDefault();
      redo();
    } else if (mod && e.key.toLowerCase() === "z") {
      e.preventDefault();
      undo();
    } else if (e.altKey && e.key === "ArrowLeft") {
      e.preventDefault();
      void go({ name: "library" });
    } else if (!mod && !e.altKey && !typing) {
      const k = e.key.toLowerCase();
      if (k === "f") viewer.current?.fit();
      else if (k === "h" || k === "0") viewer.current?.view("iso");
      else if (k === "1") viewer.current?.view("front");
      else if (k === "2") viewer.current?.view("top");
      else if (k === "3") viewer.current?.view("right");
      else if (k === "p") setOrtho((o) => !o);
      else if (k === "m") setMeasuring((m) => !m);
      else if (k === "x") setSection((s) => ({ ...s, on: !s.on }));
      else if (k === "e") setExploded((x) => !x);
      else if (k === "d") setDisplay((d) => DISPLAY[(DISPLAY.findIndex((x) => x.value === d) + 1) % DISPLAY.length].value);
      else if (k === "escape") {
        setMeasuring(false);
        viewer.current?.clearMeasure();
      }
    }
  };
  useEffect(() => {
    const h = (e: KeyboardEvent) => keys.current(e);
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  }, []);

  useEffect(() => {
    if (!measuring) viewer.current?.clearMeasure();
  }, [measuring]);

  const meshes = evaluation?.meshes.length ? evaluation.meshes : lastGood?.meshes ?? [];
  const sectionRange = useMemo(() => {
    if (!bounds) return { min: -50, max: 50 };
    const i = section.axis === "x" ? 0 : section.axis === "y" ? 1 : 2;
    return { min: Math.floor(bounds.min[i]), max: Math.ceil(bounds.max[i]) };
  }, [bounds, section.axis]);
  const onBounds = useCallback((min: V3, max: V3) => setBounds({ min, max }), []);
  const pending = detail?.pending ?? [];

  return (
    <div className="screen editor">
      <header className="topbar">
        <button type="button" className="btn ghost" onClick={() => void go({ name: "library" })} title="Back to the library (Alt+←)">
          <Icon name="back" />
          Library
        </button>
        <div className="divider" />
        <div className="title-block">
          <span className="doc-title">{name}</span>
          <span className="pill">v{version}</span>
          {dirty ? (
            <span className="unsaved">
              <span className="dot warn" />
              {restoredFrom ? `Restored v${restoredFrom}, not saved` : "Unsaved changes"}
            </span>
          ) : (
            <span className="muted small">{detail ? `Saved ${timeAgo(detail.meta.modified)}` : ""}</span>
          )}
          {busy && <span className="muted small">updating…</span>}
        </div>
        <div className="grow" />
        <button type="button" className="icon-btn" aria-label="Undo (Ctrl+Z)" title="Undo (Ctrl+Z)" disabled={!history.current.past.length} onClick={undo}>
          <Icon name="undo" />
        </button>
        <button type="button" className="icon-btn" aria-label="Redo (Ctrl+Y)" title="Redo (Ctrl+Y)" disabled={!history.current.future.length} onClick={redo}>
          <Icon name="redo" />
        </button>
        <div className="menu-anchor">
          <button type="button" className={`btn ${versionsOpen ? "on" : ""}`} aria-expanded={versionsOpen} onClick={() => setVersionsOpen((o) => !o)}>
            <Icon name="history" size={19} />
            Versions
          </button>
          {versionsOpen && detail && (
            <div className="menu right versions" role="dialog" aria-label="Versions">
              <div className="menu-title">Versions</div>
              {detail.versions.map((v) => (
                <div key={v.file} className={`version-row ${v.num === version && !v.rejected ? "current" : ""}`}>
                  <span className="vnum">v{v.num}</span>
                  <span className="grow">
                    <span className="vmsg">
                      {v.author === "claude" ? "Claude: " : "You: "}
                      {v.message || "Saved"}
                      {v.rejected && <span className="tag removed">not accepted</span>}
                    </span>
                    <span className="muted small">{new Date(v.created).toLocaleString()}</span>
                  </span>
                  {v.num === version && !v.rejected ? (
                    <span className="muted small">Current</span>
                  ) : (
                    <button type="button" className="btn small" onClick={() => void restore(v.file, v.num)}>
                      Open
                    </button>
                  )}
                  <button
                    type="button"
                    className="icon-btn small"
                    aria-label={`Open v${v.num} as a copy`}
                    title="Open as a new design"
                    onClick={async () => {
                      setVersionsOpen(false);
                      const nid = await api.duplicate(id, v.file, `${name} (v${v.num})`);
                      bumpLibrary();
                      void go({ name: "editor", id: nid });
                    }}
                  >
                    <Icon name="copy" size={17} />
                  </button>
                </div>
              ))}
            </div>
          )}
        </div>
        <button type="button" className={`btn ${dirty ? "primary" : ""}`} disabled={!dirty} onClick={() => void save()} title="Save a new version (Ctrl+S)">
          Save
          <span className="kbd">Ctrl+S</span>
        </button>
        <MenuButton
          className="btn"
          align="right"
          button={
            <>
              <Icon name="download" size={19} />
              Export
              <Icon name="chevronDown" size={16} />
            </>
          }
          items={exportItems}
        />
        <button type="button" className="icon-btn" aria-label="Settings" title="Settings" onClick={() => void go({ name: "settings", back: { name: "editor", id } })}>
          <Icon name="settings" />
        </button>
      </header>

      {pending.length > 0 && !compare && (
        <button type="button" className="banner accent flat" onClick={() => void openPending()}>
          <Icon name="sparkle" size={20} />
          <span className="grow">
            <b>Claude sent an update</b>
            {pending[pending.length - 1].message ? ` · “${pending[pending.length - 1].message}”` : ""} <span className="muted">· {timeAgo(pending[pending.length - 1].created)}</span>
          </span>
          <span className="accent-text strong">Review side by side</span>
        </button>
      )}

      <div className="body">
        <aside className="panel left" aria-label="Parts and checks">
          <h3 className="section-title">Parts</h3>
          {(report?.parts ?? []).map((p) => (
            <div key={p.id} className="part-row">
              <span className="swatch" style={{ background: p.color }} />
              <span className="grow part-text">
                <span className="strong">{p.name ?? p.id}</span>
                <span className="muted small">{fmtSize(p.size, settings.units)}</span>
              </span>
              <button
                type="button"
                className="icon-btn small ghost"
                aria-label={hidden.has(p.id) ? `Show ${p.name ?? p.id}` : `Hide ${p.name ?? p.id}`}
                aria-pressed={hidden.has(p.id)}
                onClick={() =>
                  setHidden((h) => {
                    const n = new Set(h);
                    if (n.has(p.id)) n.delete(p.id);
                    else n.add(p.id);
                    return n;
                  })
                }
              >
                <Icon name={hidden.has(p.id) ? "eyeOff" : "eye"} size={18} />
              </button>
            </div>
          ))}

          <h3 className="section-title">Checks</h3>
          {fatal && <div className="issue error">{fatal}</div>}
          {report && issues.length === 0 && (
            <div className="check-ok">
              <Icon name="check" size={18} />
              <span>
                Builds without problems
                <br />
                <span className="muted small">
                  Fits the {settings.printer.name} ({settings.printer.bed[0]} × {settings.printer.bed[1]} mm)
                </span>
              </span>
            </div>
          )}
          {issues.slice(0, 8).map((i, k) => (
            <div key={k} className={`issue ${i.severity}`}>
              <b>{i.severity === "error" ? "Error" : "Note"}</b> {i.message}
              {i.when && <em> (when {i.when})</em>}
              {i.hint && <div className="hint">{i.hint}</div>}
            </div>
          ))}
          {issues.length > 0 && (
            <button type="button" className="btn small" onClick={copyFixIt}>
              <Icon name="copy" size={16} />
              Copy fix-it message for Claude
            </button>
          )}
          <button
            type="button"
            className="btn small"
            disabled={checkAll === "busy" || !report}
            onClick={async () => {
              setCheckAll("busy");
              try {
                const r = await api.checkAll(designText, values, opts);
                setCheckAll(r);
                ui.toast(r.ok ? "Every slider value builds correctly." : `${r.errors.length} problem(s) at some slider values.`, { kind: r.ok ? "ok" : "error" });
              } catch (e) {
                setCheckAll(null);
                ui.toast(String(e), { kind: "error" });
              }
            }}
          >
            <Icon name="check" size={16} />
            {checkAll === "busy" ? "Checking every slider value…" : "Check every slider value"}
          </button>

          <div className="grow" />
          <div className="claude-note">
            <Icon name="sparkle" size={18} className="accent-text" />
            <span>
              Claude can see this design (v{version}{dirty ? ", with your unsaved values" : ""}). Ask it for a change and you get a side-by-side compare.
            </span>
          </div>
        </aside>

        <section className="stage" aria-label="3D viewer">
          <Viewer
            meshes={meshes}
            hidden={hidden}
            exploded={exploded}
            display={display}
            ortho={ortho}
            plate={plate}
            bed={settings.printer.bed}
            measuring={measuring}
            section={section}
            colors={colors}
            frameKey={frameKey}
            units={settings.units}
            onMeasure={setMeasure}
            onBounds={onBounds}
            handleRef={viewer}
            measureLabel={measure ? fmtLen(measure.dist, settings.units, 2) : undefined}
          />
          <div className="view-buttons">
            <button type="button" className="icon-btn" aria-label="Home view (H)" title="Home view (H)" onClick={() => viewer.current?.view("iso")}>
              <Icon name="home" size={18} />
            </button>
            <button type="button" className="icon-btn" aria-label="Fit to view (F)" title="Fit to view (F)" onClick={() => viewer.current?.fit()}>
              <Icon name="fit" size={18} />
            </button>
          </div>

          <div className="float-cards">
          {measuring && (
            <div className="float-card">
              <span className="section-title">Measure</span>
              {measure ? (
                <>
                  <span className="big-number">{fmtLen(measure.dist, settings.units, 2)}</span>
                  <span className="muted small">
                    ΔX {fmtLen(measure.d[0], settings.units, 2)} · ΔY {fmtLen(measure.d[1], settings.units, 2)} · ΔZ {fmtLen(measure.d[2], settings.units, 2)}
                  </span>
                </>
              ) : (
                <span className="muted">Click two points on the model. Corners snap.</span>
              )}
              <span className="muted small">Esc or M to finish</span>
            </div>
          )}
          {section.on && (
            <div className="float-card">
              <span className="section-title">Section</span>
              <Segmented
                label="Section axis"
                value={section.axis}
                onChange={(axis) => setSection((s) => ({ ...s, axis, at: 0 }))}
                options={[
                  { value: "x", label: "X" },
                  { value: "y", label: "Y" },
                  { value: "z", label: "Z" },
                ]}
              />
              <label className="field">
                <span>
                  Cut at {section.axis} = {fmtLen(section.at, settings.units, 1)}
                </span>
                <input
                  type="range"
                  className="param-slider"
                  min={sectionRange.min}
                  max={sectionRange.max}
                  step={0.5}
                  value={Math.min(Math.max(section.at, sectionRange.min), sectionRange.max)}
                  onChange={(e) => setSection((s) => ({ ...s, at: Number(e.target.value) }))}
                />
              </label>
            </div>
          )}
          </div>

          <div className="toolbar" role="toolbar" aria-label="View tools">
            <MenuButton
              up
              className="tb-menu"
              title="Display mode (D)"
              button={
                <>
                  <Icon name="box" size={17} />
                  {DISPLAY.find((d) => d.value === display)?.label}
                  <Icon name="chevronDown" size={15} />
                </>
              }
              items={DISPLAY.map((d) => ({ label: d.label, checked: display === d.value, run: () => setDisplay(d.value) }))}
            />
            <span className="tb-divider" />
            <button type="button" className={ortho ? "on" : ""} aria-pressed={ortho} onClick={() => setOrtho((o) => !o)} title="Orthographic projection (P)">
              Ortho
            </button>
            <button type="button" className={plate ? "on" : ""} aria-pressed={plate} onClick={() => setPlate((p) => !p)} title="Show the printer's build plate">
              <Icon name="plate" size={17} />
              Plate
            </button>
            <button type="button" className={exploded ? "on" : ""} aria-pressed={exploded} onClick={() => setExploded((x) => !x)} title="Pull parts apart (E)">
              <Icon name="layers" size={17} />
              Exploded
            </button>
            <span className="tb-divider" />
            <button type="button" className={section.on ? "on" : ""} aria-pressed={section.on} onClick={() => setSection((s) => ({ ...s, on: !s.on }))} title="Section view (X)">
              <Icon name="section" size={17} />
              Section
            </button>
            <button type="button" className={measuring ? "on" : ""} aria-pressed={measuring} onClick={() => setMeasuring((m) => !m)} title="Measure (M)">
              <Icon name="ruler" size={17} />
              Measure
            </button>
          </div>
        </section>

        <aside className="panel right" aria-label="Parameters">
          <div className="panel-head">
            <div className="row between">
              <h2>Parameters</h2>
              <Segmented
                label="Parameter detail"
                value={advanced ? "adv" : "simple"}
                onChange={(v) => setAdvanced(v === "adv")}
                options={[
                  { value: "simple", label: "Simple" },
                  { value: "adv", label: "Advanced" },
                ]}
              />
            </div>
            <div className="row end">
              <label className="field grow">
                <span>Preset</span>
                <select
                  value={activePreset?.name ?? ""}
                  onChange={(e) => {
                    const p = presets.find((x) => x.name === e.target.value);
                    if (p) applyParams(p.params);
                  }}
                >
                  <option value="" disabled>
                    {presets.length ? "Custom values" : "No presets yet"}
                  </option>
                  {presets.map((p) => (
                    <option key={p.name} value={p.name}>
                      {p.name}
                    </option>
                  ))}
                </select>
              </label>
              <MenuButton className="icon-btn" align="right" ariaLabel="Preset options" title="Preset options" button={<Icon name="more" size={18} />} items={presetMenu} />
            </div>
          </div>
          <div className="panel-scroll">
            {report && report.params.length > 0 ? (
              <ParamPanel params={report.params} values={values} saved={saved} advanced={advanced} units={settings.units} onChange={setParam} />
            ) : (
              <p className="muted">{report ? "This design has no parameters." : "Loading…"}</p>
            )}
            {!advanced && (report?.params.some((p) => p.advanced) ?? false) && (
              <button type="button" className="link" onClick={() => setAdvanced(true)}>
                Show {report!.params.filter((p) => p.advanced).length} advanced settings (tolerances, walls…)
              </button>
            )}
          </div>
        </aside>
      </div>

      <footer className="statusbar">
        <span>
          {settings.printer.name} · {settings.printer.bed[0]} × {settings.printer.bed[1]} mm · {settings.printer.nozzle} mm nozzle
        </span>
        <span>Units: {settings.units === "in" ? "inches" : "mm"}</span>
        <span className="grow" />
        <span>
          {api.kind === "desktop" ? "Native engine" : "Browser engine"}
          {report ? ` · ${report.parts.length} part${report.parts.length === 1 ? "" : "s"}` : ""}
        </span>
      </footer>

      {compare && lastGood && (
        <Compare
          name={name}
          version={version}
          current={lastGood}
          values={values}
          pending={compare}
          onClose={() => setCompare(null)}
          onAccept={async (text, params) => {
            await api.saveVersion(id, text, params, "claude", compare.message || "Update from Claude");
            await api.removePending(id, compare.file);
            setCompare(null);
            history.current = { past: [], future: [], last: null };
            needsThumb.current = true;
            normalize.current = { params, keepSaved: false };
            setTick((t) => t + 1);
            setRestoredFrom(null);
            setDesignText(text);
            setSavedText(text);
            setSaved(params);
            await reloadDetail();
            bumpLibrary();
            ui.toast("Update accepted and saved", { kind: "ok" });
          }}
          onSaveAsNew={async (text, params) => {
            const newName = await ui.prompt("Save as a new design", "Name of the new design", `${name} (2)`, "Save");
            if (!newName) return;
            let designText = text;
            try {
              const j = JSON.parse(text);
              j.meta = { ...(j.meta ?? {}), name: newName };
              designText = JSON.stringify(j, null, 2);
            } catch {
              // keep the text as it is; the library names it from the design
            }
            const nid = await api.create(designText, params, "claude", compare.message || "Update from Claude, saved as a new design");
            await api.removePending(id, compare.file);
            setCompare(null);
            await reloadDetail();
            bumpLibrary();
            ui.toast("Saved as a new design", { kind: "ok", action: { label: "Open", run: () => void go({ name: "editor", id: nid }) } });
          }}
          onReject={async (text, params) => {
            await api.saveVersion(id, text, params, "claude", compare.message || "Update from Claude", true);
            await api.removePending(id, compare.file);
            setCompare(null);
            await reloadDetail();
            ui.toast("Update rejected. It stays in Versions.", { kind: "info" });
          }}
        />
      )}
    </div>
  );
}

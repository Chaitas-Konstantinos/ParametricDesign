import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { loadEngine, type Engine, type Evaluation, type OpenedDesign, type ParamValue, type Params, type Report } from "./engine";
import { ParamPanel } from "./ParamPanel";
import { Viewer } from "./Viewer";
import { EXAMPLES } from "./examples";

function fmt(v: number) {
  return String(Math.round(v * 10) / 10);
}

function slug(s: string) {
  return s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "design";
}

export function App() {
  const [engine, setEngine] = useState<Engine | null>(null);
  const [designText, setDesignText] = useState(EXAMPLES[0].text);
  const [designKey, setDesignKey] = useState("example-0");
  const [values, setValues] = useState<Params>({});
  const [evaluation, setEvaluation] = useState<Evaluation | null>(null);
  const [fatal, setFatal] = useState<string | null>(null);
  const [advanced, setAdvanced] = useState(false);
  const [exploded, setExploded] = useState(false);
  const [busy, setBusy] = useState(false);
  const [status, setStatus] = useState("");
  const [checkReport, setCheckReport] = useState<Report | null>(null);
  const [pasting, setPasting] = useState(false);
  const [connecting, setConnecting] = useState(false);
  const [pdPath, setPdPath] = useState<string | null>(null);
  const [paste, setPaste] = useState("");
  const lastGood = useRef<Evaluation | null>(null);

  useEffect(() => {
    loadEngine().then(setEngine, (e) => setFatal(`Could not start the engine: ${e}`));
  }, []);

  const load = useCallback((d: OpenedDesign) => {
    setDesignText(d.text);
    setValues(d.params);
    setDesignKey(`${d.fileName}-${Date.now()}`);
    setCheckReport(null);
    setStatus(`Opened ${d.fileName}`);
  }, []);

  // designs sent by an LLM through the MCP server
  useEffect(() => {
    if (!engine) return;
    let off: (() => void) | undefined;
    engine.onInbox(load).then((f) => (off = f));
    return () => off?.();
  }, [engine, load]);

  // rebuild whenever the design or a slider changes
  useEffect(() => {
    if (!engine) return;
    let cancelled = false;
    const t = setTimeout(async () => {
      setBusy(true);
      try {
        const ev = await engine.evaluate(designText, values);
        if (cancelled) return;
        setFatal(null);
        setEvaluation(ev);
        if (ev.meshes.length) lastGood.current = ev;
      } catch (e) {
        if (!cancelled) setFatal(String(e));
      } finally {
        if (!cancelled) setBusy(false);
      }
    }, 90);
    return () => {
      cancelled = true;
      clearTimeout(t);
    };
  }, [engine, designText, values]);

  const report = evaluation?.report;
  const meshes = evaluation?.meshes.length ? evaluation.meshes : lastGood.current?.meshes ?? [];
  const name = report?.name || "design";
  const errors = report?.errors ?? [];
  const warnings = report?.warnings ?? [];
  const hasAdvanced = useMemo(() => report?.params.some((p) => p.advanced) ?? false, [report]);

  const setParam = (n: string, v: ParamValue) => {
    setValues((old) => ({ ...old, [n]: v }));
    setCheckReport(null);
  };

  const doExport = async (format: "stl" | "3mf") => {
    if (!engine) return;
    try {
      const where = await engine.saveExport(designText, values, format, slug(name));
      if (where) setStatus(`Exported ${where}`);
    } catch (e) {
      setStatus(`Export failed: ${e}`);
    }
  };

  const doCheck = async () => {
    if (!engine) return;
    setStatus("Checking every size range...");
    try {
      const r = await engine.checkAll(designText, values);
      setCheckReport(r);
      setStatus(r.ok ? "All parameter ranges build correctly." : `${r.errors.length} problem(s) at some parameter values.`);
    } catch (e) {
      setStatus(String(e));
    }
  };

  const doOpen = async () => {
    if (!engine) return;
    try {
      const d = await engine.openFile();
      if (d) load(d);
    } catch (e) {
      setStatus(`Could not open: ${e}`);
    }
  };

  const issues = [...errors, ...warnings, ...(checkReport?.errors.filter((e) => e.when) ?? [])];

  return (
    <div className="app">
      <header>
        <div className="brand">ParametricDesign</div>
        <button onClick={doOpen}>Open…</button>
        <button onClick={() => setPasting(true)}>Paste JSON</button>
        <button
          onClick={async () => {
            setPdPath(engine ? await engine.mcpServerPath() : null);
            setConnecting(true);
          }}
        >
          Connect AI
        </button>
        <select
          value=""
          onChange={(e) => {
            const ex = EXAMPLES[Number(e.target.value)];
            if (ex) load({ text: ex.text, params: {}, fileName: ex.name });
          }}
        >
          <option value="">Examples…</option>
          {EXAMPLES.map((ex, i) => (
            <option key={ex.name} value={i}>
              {ex.name}
            </option>
          ))}
        </select>
        <div className="spacer" />
        <div className="seg" role="group" aria-label="Mode">
          <button className={!advanced ? "on" : ""} onClick={() => setAdvanced(false)}>
            Simple
          </button>
          <button className={advanced ? "on" : ""} onClick={() => setAdvanced(true)}>
            Advanced
          </button>
        </div>
        <label className="toggle">
          <input type="checkbox" checked={exploded} onChange={(e) => setExploded(e.target.checked)} /> Exploded
        </label>
        <button onClick={doCheck} disabled={!report?.ok}>
          Check all sizes
        </button>
        <button className="primary" onClick={() => doExport("3mf")} disabled={!report?.ok} title="Parts, colours and the editable design in one file">
          Export 3MF
        </button>
        <button onClick={() => doExport("stl")} disabled={!report?.ok}>
          STL
        </button>
      </header>
      <main>
        <section className="stage">
          <Viewer meshes={meshes} exploded={exploded} designKey={designKey} />
          <div className="title">
            <h1>{name}</h1>
            {busy && <span className="muted">updating…</span>}
          </div>
          {report && report.parts.length > 0 && (
            <ul className="parts">
              {report.parts.map((p) => (
                <li key={p.id}>
                  <span className="swatch" style={{ background: p.color }} />
                  <b>{p.name ?? p.id}</b> {fmt(p.size[0])} × {fmt(p.size[1])} × {fmt(p.size[2])} mm
                </li>
              ))}
            </ul>
          )}
          {(fatal || issues.length > 0) && (
            <div className="issues">
              {fatal && <div className="issue error">{fatal}</div>}
              {issues.length > 0 && (
                <button
                  className="copy-fix"
                  onClick={async () => {
                    const lines = issues.map((i) => `- ${i.severity.toUpperCase()} at ${i.path || "(design)"}: ${i.message}${i.when ? ` (when ${i.when})` : ""}${i.hint ? `\n  fix: ${i.hint}` : ""}`);
                    const msg = `ParametricDesign reports these problems with the design. Please fix all of them and send the complete corrected JSON:\n\n${lines.join("\n")}`;
                    await navigator.clipboard.writeText(msg);
                    setStatus("Copied a fix-it message: paste it into your LLM chat.");
                  }}
                >
                  Copy fix-it message for your LLM
                </button>
              )}
              {issues.slice(0, 8).map((i, k) => (
                <div key={k} className={`issue ${i.severity}`}>
                  <b>{i.severity === "error" ? "Error" : "Note"}</b> {i.message}
                  {i.when && <em> (when {i.when})</em>}
                  {i.hint && <div className="hint">{i.hint}</div>}
                </div>
              ))}
            </div>
          )}
        </section>
        <aside>
          <h2>Parameters</h2>
          {report?.params.length ? (
            <ParamPanel params={report.params} values={values} advanced={advanced} onChange={setParam} onReset={() => setValues({})} />
          ) : (
            <p className="muted">This design has no parameters.</p>
          )}
          {!advanced && hasAdvanced && (
            <button className="link" onClick={() => setAdvanced(true)}>
              Show tolerances and fine-tuning (Advanced)
            </button>
          )}
        </aside>
      </main>
      <footer>
        <span>{engine ? (engine.kind === "desktop" ? "Native engine" : "Browser engine (WebAssembly)") : "Starting engine…"}</span>
        <span className="muted">{status}</span>
      </footer>
      {connecting && (
        <div className="modal" onClick={() => setConnecting(false)}>
          <div className="dialog" onClick={(e) => e.stopPropagation()}>
            <h2>Connect an AI assistant</h2>
            {pdPath ? (
              <>
                <p className="muted">
                  The MCP server lets Claude (or any MCP client) check, preview and open designs in this app. Designs it opens appear here automatically.
                </p>
                <h3>Claude Code (run in a terminal)</h3>
                <pre className="code">{`claude mcp add --scope user parametricdesign -- "${pdPath}" mcp`}</pre>
                <h3>Claude Desktop (Settings → Developer → Edit Config)</h3>
                <pre className="code">{JSON.stringify({ mcpServers: { parametricdesign: { command: pdPath, args: ["mcp"] } } }, null, 2)}</pre>
                <p className="muted">Then ask for a design, e.g. “Design a snap-lid box for 10 SD cards and open it in ParametricDesign.”</p>
              </>
            ) : (
              <p className="muted">
                The MCP server (<code>pd</code>) was not found next to the app. It ships with the installed desktop app; in a browser, or in a development build, run <code>pd mcp</code> from a build of the
                command line instead.
              </p>
            )}
            <div className="row">
              {pdPath && (
                <button onClick={() => navigator.clipboard.writeText(`claude mcp add --scope user parametricdesign -- "${pdPath}" mcp`).then(() => setStatus("Copied the Claude Code command."))}>
                  Copy Claude Code command
                </button>
              )}
              <button className="primary" onClick={() => setConnecting(false)}>
                Close
              </button>
            </div>
          </div>
        </div>
      )}
      {pasting && (
        <div className="modal" onClick={() => setPasting(false)}>
          <div className="dialog" onClick={(e) => e.stopPropagation()}>
            <h2>Paste a design</h2>
            <p className="muted">Paste the JSON your LLM wrote (pd/0.1 format). Errors are listed with fixes you can send back to it.</p>
            <textarea value={paste} onChange={(e) => setPaste(e.target.value)} placeholder='{"format": "pd/0.1", ...}' autoFocus />
            <div className="row">
              <button onClick={() => setPasting(false)}>Cancel</button>
              <button
                className="primary"
                onClick={() => {
                  const m = paste.match(/```(?:json)?\s*([\s\S]*?)```/);
                  load({ text: (m ? m[1] : paste).trim(), params: {}, fileName: "pasted design" });
                  setPasting(false);
                  setPaste("");
                }}
              >
                Open
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

import { useEffect, useState } from "react";
import { useApp, type Route } from "../App";
import { Icon } from "../components/Icon";
import { Segmented, useUi } from "../components/ui";
import { ACCENTS, PRINTERS, type PrinterSetting, type Settings } from "../settings";

const SHORTCUTS: [string, string][] = [
  ["Save a new version", "Ctrl+S"],
  ["Undo / redo", "Ctrl+Z / Ctrl+Y"],
  ["Export 3MF", "Ctrl+E"],
  ["Back to the library", "Alt+←"],
  ["Search the library", "Ctrl+F"],
  ["Fit to view", "F"],
  ["Home (iso) view", "H or 0"],
  ["Front / top / right view", "1 / 2 / 3"],
  ["Perspective ↔ orthographic", "P"],
  ["Display mode", "D"],
  ["Measure", "M"],
  ["Section", "X"],
  ["Exploded view", "E"],
  ["Stop measuring", "Esc"],
];

export function SettingsScreen({ welcome, back }: { welcome?: boolean; back?: Route }) {
  const { api, settings, saveSettings, go, bumpLibrary } = useApp();
  const ui = useUi();
  const [s, setS] = useState<Settings>(settings);
  const [folder, setFolder] = useState(settings.libraryPath ?? settings.defaultLibraryPath ?? "");
  const [examples, setExamples] = useState(true);
  const [pdPath, setPdPath] = useState<string | null>(null);
  const [custom, setCustom] = useState<PrinterSetting>(settings.printer.id === "custom" ? settings.printer : { id: "custom", name: "Custom printer", bed: [220, 220, 250], nozzle: 0.4 });

  useEffect(() => {
    api.mcpServerPath().then(setPdPath);
  }, [api]);

  // appearance changes apply right away
  const live = async (patch: Partial<Settings>) => {
    const next = { ...s, ...patch };
    setS(next);
    await saveSettings({ ...next, libraryPath: settings.libraryPath });
  };

  const finish = async () => {
    if (!folder.trim()) {
      ui.toast("Choose a folder for your library first.", { kind: "error" });
      return;
    }
    try {
      const changed = folder !== settings.libraryPath;
      await saveSettings({ ...s, libraryPath: folder });
      if (welcome && examples) await api.addExamples();
      if (changed) bumpLibrary();
      await go(welcome ? { name: "library" } : back ?? { name: "library" });
    } catch (e) {
      ui.toast(String(e), { kind: "error" });
    }
  };

  const claudeCode = pdPath ? `claude mcp add --scope user parametricdesign -- "${pdPath}" mcp` : "";
  const claudeDesktop = pdPath ? JSON.stringify({ mcpServers: { parametricdesign: { command: pdPath, args: ["mcp"] } } }, null, 2) : "";
  const copy = async (t: string) => {
    await navigator.clipboard.writeText(t);
    ui.toast("Copied", { kind: "ok" });
  };

  return (
    <div className="screen">
      <header className="topbar">
        {welcome ? (
          <div className="brand">
            <span className="brand-mark">
              <Icon name="logo" size={22} />
            </span>
            Welcome to ParametricDesign
          </div>
        ) : (
          <>
            <button type="button" className="btn ghost" onClick={() => void finish()}>
              <Icon name="back" />
              {back?.name === "editor" ? "Back to the design" : back?.name === "ask" ? "Design with AI" : "Library"}
            </button>
            <h1 className="h-inline">Settings</h1>
          </>
        )}
      </header>
      <main className="settings">
        {welcome && <p className="lead">Two quick choices and you're ready. You can change both later in Settings.</p>}

        <section className="card-panel">
          <h2>{welcome ? "1 · Where should your library live?" : "Library folder"}</h2>
          <p className="muted">Designs, versions, presets and previews are saved here. Choose a folder inside OneDrive or Dropbox to use the same library on several computers.</p>
          <div className="row wrap">
            <label className="field grow">
              <span className="sr-only">Library folder</span>
              <input value={folder} onChange={(e) => setFolder(e.target.value)} disabled={api.kind === "browser"} />
            </label>
            <button
              type="button"
              className="btn"
              disabled={api.kind === "browser"}
              onClick={async () => {
                const p = await api.pickFolder(folder);
                if (p) setFolder(p);
              }}
            >
              <Icon name="folder" size={18} />
              Choose folder…
            </button>
          </div>
          <div className="note">
            <Icon name="info" size={18} />
            <span>If two computers change the same design at the same time, both are kept: the other one shows up as an extra version. Designs Claude sends land in this library too.</span>
          </div>
          {welcome && (
            <label className="check-row">
              <input type="checkbox" checked={examples} onChange={(e) => setExamples(e.target.checked)} />
              Add 5 example designs to start from
            </label>
          )}
        </section>

        <section className="card-panel">
          <h2>{welcome ? "2 · Which printer do you use?" : "Printer"}</h2>
          <p className="muted">Used for the “fits the bed” check, the build plate in the viewer and the plate layout in 3MF exports.</p>
          <div className="printer-grid">
            {[...PRINTERS, custom].map((p) => (
              <button key={p.id} type="button" className={`printer ${s.printer.id === p.id ? "on" : ""}`} aria-pressed={s.printer.id === p.id} onClick={() => void live({ printer: p })}>
                <span className="strong">{p.id === "custom" ? "Custom printer" : p.name}</span>
                <span className="muted small">
                  {p.bed[0]} × {p.bed[1]} × {p.bed[2]} mm
                </span>
              </button>
            ))}
          </div>
          {s.printer.id === "custom" && (
            <div className="row wrap">
              {(["X", "Y", "Z"] as const).map((axis, i) => (
                <label key={axis} className="field small-field">
                  <span>Bed {axis} (mm)</span>
                  <input
                    type="number"
                    min={50}
                    max={1000}
                    value={custom.bed[i]}
                    onChange={(e) => {
                      const bed = [...custom.bed] as [number, number, number];
                      bed[i] = Math.max(10, Number(e.target.value) || 0);
                      const c = { ...custom, bed };
                      setCustom(c);
                      void live({ printer: c });
                    }}
                  />
                </label>
              ))}
            </div>
          )}
          <label className="inline-field">
            <span>Nozzle</span>
            <select value={s.printer.nozzle} onChange={(e) => void live({ printer: { ...s.printer, nozzle: Number(e.target.value) } })}>
              {[0.2, 0.4, 0.6, 0.8].map((n) => (
                <option key={n} value={n}>
                  {n} mm
                </option>
              ))}
            </select>
          </label>
        </section>

        <div className="row wrap align-start">
          <section className="card-panel grow">
            <h2>Appearance</h2>
            <div className="field">
              <span>Theme</span>
              <Segmented
                label="Theme"
                value={s.theme}
                onChange={(theme) => void live({ theme })}
                options={[
                  { value: "system", label: "Follow Windows" },
                  { value: "light", label: "Light" },
                  { value: "dark", label: "Dark" },
                ]}
              />
            </div>
            <div className="field">
              <span>Accent colour</span>
              <div className="swatches">
                {ACCENTS.map((a) => (
                  <button key={a.hex} type="button" className={`swatch-btn ${s.accent === a.hex ? "on" : ""}`} style={{ background: a.hex }} aria-label={`${a.name} accent`} aria-pressed={s.accent === a.hex} onClick={() => void live({ accent: a.hex })} />
                ))}
                <label className="swatch-custom" title="Custom colour">
                  <input type="color" value={s.accent} aria-label="Custom accent colour" onChange={(e) => void live({ accent: e.target.value })} />
                </label>
              </div>
            </div>
            <div className="field">
              <span>Text size</span>
              <Segmented
                label="Text size"
                value={s.textSize}
                onChange={(textSize) => void live({ textSize })}
                options={[
                  { value: "normal", label: "Normal" },
                  { value: "large", label: "Large" },
                ]}
              />
            </div>
            <div className="field">
              <span>Show sizes in</span>
              <Segmented
                label="Units"
                value={s.units}
                onChange={(units) => void live({ units })}
                options={[
                  { value: "mm", label: "Millimetres" },
                  { value: "in", label: "Inches" },
                ]}
              />
            </div>
          </section>

          <section className="card-panel grow">
            <h2>Claude</h2>
            {pdPath ? (
              <>
                <p className="muted">Connect Claude so it can check designs, preview them and send them here. In Claude Code, run this once in a terminal:</p>
                <pre className="code">{claudeCode}</pre>
                <button type="button" className="btn small" onClick={() => void copy(claudeCode)}>
                  <Icon name="copy" size={16} />
                  Copy Claude Code command
                </button>
                <p className="muted">For Claude Desktop, add this in Settings → Developer → Edit Config:</p>
                <pre className="code">{claudeDesktop}</pre>
                <button type="button" className="btn small" onClick={() => void copy(claudeDesktop)}>
                  <Icon name="copy" size={16} />
                  Copy Claude Desktop config
                </button>
              </>
            ) : (
              <p className="muted">
                The MCP server (<code>pd</code>) was not found next to the app. It comes with the installed app; in a development build run <code>pd mcp</code> from your build of the command line.
              </p>
            )}
          </section>
        </div>

        {!welcome && (
          <section className="card-panel">
            <h2>Keyboard shortcuts</h2>
            <div className="shortcuts">
              {SHORTCUTS.map(([what, key]) => (
                <div key={what} className="shortcut">
                  <span>{what}</span>
                  <kbd>{key}</kbd>
                </div>
              ))}
            </div>
          </section>
        )}

        <div className="row end">
          <button type="button" className="btn primary big" onClick={() => void finish()}>
            {welcome ? "Start" : "Done"}
          </button>
        </div>
      </main>
    </div>
  );
}

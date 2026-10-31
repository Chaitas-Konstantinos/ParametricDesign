import { Fragment, useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../App";
import type { DesignRequest, RequestPhoto, SkillCatalog, SkillInfo } from "../api";
import { Icon } from "../components/Icon";
import { MenuButton, useUi } from "../components/ui";
import { fmtSize } from "../settings";

interface Draft {
  title: string;
  brief: string;
  photos: RequestPhoto[];
  /** Skills the user ticked; what they require is added on top. */
  optional: string[];
  withPrinter: boolean;
}

const EMPTY: Draft = { title: "", brief: "", photos: [], optional: [], withPrinter: true };

// The request survives leaving the screen while the app runs.
let draft: Draft = EMPTY;
let catalogCache: SkillCatalog | null = null;

const MAX_PHOTOS = 10;
// The size vision models look at; keeps requests and skill files small.
const MAX_SIDE = 1568;

async function readPhoto(file: File): Promise<RequestPhoto> {
  const bmp = await createImageBitmap(file, { imageOrientation: "from-image" });
  const scale = Math.min(1, MAX_SIDE / Math.max(bmp.width, bmp.height));
  const canvas = document.createElement("canvas");
  canvas.width = Math.max(1, Math.round(bmp.width * scale));
  canvas.height = Math.max(1, Math.round(bmp.height * scale));
  const ctx = canvas.getContext("2d")!;
  ctx.fillStyle = "#ffffff"; // transparent screenshots turn white, not black
  ctx.fillRect(0, 0, canvas.width, canvas.height);
  ctx.drawImage(bmp, 0, 0, canvas.width, canvas.height);
  bmp.close();
  const base = (file.name || "photo").replace(/\.[^.]*$/, "");
  return { name: `${base}.jpg`, caption: "", dataUrl: canvas.toDataURL("image/jpeg", 0.88) };
}

/** Puts a photo on the clipboard so it can be pasted into a chat. */
async function copyImage(dataUrl: string) {
  const img = new Image();
  img.src = dataUrl;
  await img.decode();
  const canvas = document.createElement("canvas");
  canvas.width = img.naturalWidth;
  canvas.height = img.naturalHeight;
  canvas.getContext("2d")!.drawImage(img, 0, 0);
  const png = await new Promise<Blob>((ok, fail) => canvas.toBlob((b) => (b ? ok(b) : fail(new Error("could not copy the photo"))), "image/png"));
  await navigator.clipboard.write([new ClipboardItem({ "image/png": png })]);
}

/** The picked skills plus everything they require, in catalog order. */
function withRequirements(catalog: SkillInfo[], picked: string[]): string[] {
  const byId = new Map(catalog.map((s) => [s.id, s]));
  const set = new Set<string>();
  const add = (id: string) => {
    const s = byId.get(id);
    if (!s || s.core || set.has(id)) return;
    set.add(id);
    s.requires.forEach(add);
  };
  picked.forEach(add);
  return catalog.filter((s) => set.has(s.id)).map((s) => s.id);
}

/** The user's picks after ticking a skill on or off. Ticking one drops any pick
 *  that would bring in another skill of its group (one connector type). */
function pick(catalog: SkillInfo[], picked: string[], id: string, on: boolean): string[] {
  if (!on) return picked.filter((p) => p !== id);
  const group = catalog.find((s) => s.id === id)?.group;
  const clashes = (p: string) => !!group && withRequirements(catalog, [p]).some((c) => c !== id && catalog.find((s) => s.id === c)?.group === group);
  return [...picked.filter((p) => p !== id && !clashes(p)), id];
}

const capitalize = (t: string) => t.charAt(0).toUpperCase() + t.slice(1);

function CopyBox({ text, label = "Copy" }: { text: string; label?: string }) {
  const ui = useUi();
  return (
    <div className="copy-box">
      <pre className="code">{text}</pre>
      <button
        type="button"
        className="btn small"
        onClick={async () => {
          await navigator.clipboard.writeText(text);
          ui.toast("Copied", { kind: "ok" });
        }}
      >
        <Icon name="copy" size={16} />
        {label}
      </button>
    </div>
  );
}

type Result = { kind: "sent"; message: string } | { kind: "skill"; starter: string; path: string } | { kind: "prompt-copied" } | { kind: "prompt-saved"; starter: string; path: string };

export function AskScreen() {
  const { api, settings, go } = useApp();
  const ui = useUi();
  const [req, setReq] = useState<Draft>(draft);
  const [skills, setSkills] = useState<SkillCatalog | null>(catalogCache);
  const catalog = skills?.skills ?? null;
  const [dragging, setDragging] = useState(false);
  const [busy, setBusy] = useState<string | null>(null);
  const [result, setResult] = useState<Result | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const photoCount = useRef(req.photos.length);
  photoCount.current = req.photos.length;
  const desktop = api.kind === "desktop";

  useEffect(() => {
    draft = req;
  }, [req]);
  // an answer shown for an older version of the request would be misleading
  useEffect(() => {
    setResult(null);
  }, [req]);

  // The skills folder is read again on every visit and whenever the window
  // gets focus, so skills added or edited there show up straight away.
  const loadSkills = async (announce = false) => {
    try {
      const c = await api.skillsCatalog();
      catalogCache = c;
      setSkills(c);
      // drop picks whose skill was removed from the folder
      setReq((r) => {
        const kept = r.optional.filter((id) => c.skills.some((s) => s.id === id && !s.core));
        return kept.length === r.optional.length ? r : { ...r, optional: kept };
      });
      if (announce) ui.toast(`${c.skills.length} skills loaded.`, { kind: "ok" });
    } catch (e) {
      ui.toast(String(e), { kind: "error" });
    }
  };
  useEffect(() => {
    void loadSkills();
    if (!desktop) return;
    const onFocus = () => void loadSkills();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [api]);

  const openSkillsFolder = () => api.openSkillsFolder().catch((e) => ui.toast(String(e), { kind: "error" }));
  const restoreSkills = async () => {
    if (!(await ui.confirm("Restore built-in skills?", "The built-in skills go back to their original text. Your changes to them are lost; skills you added are kept.", "Restore"))) return;
    try {
      const n = await api.restoreSkills();
      ui.toast(n ? `Restored ${n} files.` : "The built-in skills were already original.", { kind: "ok" });
      await loadSkills();
    } catch (e) {
      ui.toast(String(e), { kind: "error" });
    }
  };

  const update = (patch: Partial<Draft>) => setReq((r) => ({ ...r, ...patch }));

  const addFiles = async (files: File[]) => {
    const images = files.filter((f) => f.type.startsWith("image/") || /\.(jpe?g|png|webp|gif|bmp|heic|heif)$/i.test(f.name));
    if (!images.length) return;
    const room = MAX_PHOTOS - photoCount.current;
    if (room <= 0) {
      ui.toast(`Up to ${MAX_PHOTOS} photos.`, { kind: "error" });
      return;
    }
    const added: RequestPhoto[] = [];
    for (const f of images.slice(0, room)) {
      try {
        added.push(await readPhoto(f));
      } catch {
        ui.toast(`Can't read “${f.name}”. Use JPG or PNG (on an iPhone: Settings → Camera → Formats → Most Compatible).`, { kind: "error", ms: 7000 });
      }
    }
    if (images.length > room) ui.toast(`Only the first ${room} were added: up to ${MAX_PHOTOS} photos.`, { kind: "error" });
    if (added.length) setReq((r) => ({ ...r, photos: [...r.photos, ...added] }));
  };

  // paste screenshots and photos anywhere on the screen
  useEffect(() => {
    const onPaste = (e: ClipboardEvent) => {
      const files = Array.from(e.clipboardData?.files ?? []).filter((f) => f.type.startsWith("image/"));
      if (!files.length) return;
      e.preventDefault();
      void addFiles(files);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.altKey && e.key === "ArrowLeft") void go({ name: "library" });
    };
    window.addEventListener("paste", onPaste);
    window.addEventListener("keydown", onKey);
    return () => {
      window.removeEventListener("paste", onPaste);
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const printer = settings.printer;
  const selected = useMemo(() => (catalog ? withRequirements(catalog, req.optional) : []), [catalog, req.optional]);
  const request: DesignRequest = useMemo(
    () => ({
      title: req.title.trim(),
      brief: req.brief.trim(),
      optional: selected,
      photos: req.photos,
      printer: req.withPrinter ? { name: printer.id === "custom" ? "Custom printer" : printer.name, bed: printer.bed, nozzle: printer.nozzle } : null,
    }),
    [req, selected, printer],
  );
  const described = !!(request.title || request.brief || request.photos.length);

  const run = async (what: string, f: () => Promise<void>) => {
    setBusy(what);
    try {
      await f();
    } catch (e) {
      ui.toast(String(e), { kind: "error", ms: 7000 });
    } finally {
      setBusy(null);
    }
  };

  const sendToClaude = () =>
    run("send", async () => {
      if (!described) {
        ui.toast("Describe what you want first, or add a photo.", { kind: "error" });
        return;
      }
      await api.sendRequest(request);
      const message = `Design the request I just prepared in ParametricDesign${request.title ? ` (“${request.title}”)` : ""}.`;
      setResult({ kind: "sent", message });
      try {
        await navigator.clipboard.writeText(message);
        ui.toast("Sent. The message for Claude is copied: paste it into Claude.", { kind: "ok", ms: 7000 });
      } catch {
        ui.toast("Sent. Now paste the message below into Claude.", { kind: "ok" });
      }
    });

  const exportSkill = () =>
    run("skill", async () => {
      const r = await api.exportSkillBundle(request);
      if (r) setResult({ kind: "skill", starter: r.starter, path: r.path });
    });

  const copyPrompt = () =>
    run("copy", async () => {
      const p = await api.skillsPrompt(request);
      await navigator.clipboard.writeText(p.prompt);
      setResult({ kind: "prompt-copied" });
      ui.toast("Prompt copied", { kind: "ok" });
    });

  const savePrompt = () =>
    run("save", async () => {
      const p = await api.savePrompt(request);
      if (p) setResult({ kind: "prompt-saved", starter: p.starter, path: p.path });
    });

  const exportOne = (s: SkillInfo) =>
    run(`one-${s.id}`, async () => {
      const path = await api.exportSkill(s.id);
      if (path) ui.toast(`Saved “${s.title}”`, { kind: "ok" });
    });

  const core = catalog?.filter((s) => s.core) ?? [];
  // by category, categories in the order their first skill comes
  const optional = useMemo(() => {
    const rest = catalog?.filter((s) => !s.core) ?? [];
    const cats = [...new Set(rest.map((s) => s.category))];
    return cats.flatMap((c) => rest.filter((s) => s.category === c));
  }, [catalog]);
  const title = (id: string) => catalog?.find((s) => s.id === id)?.title ?? id;
  const groupSize = (g?: string | null) => (g ? optional.filter((s) => s.group === g).length : 0);
  const photosNote = request.photos.length ? `, attach your ${request.photos.length === 1 ? "photo" : `${request.photos.length} photos`}` : "";
  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

  return (
    <div className="screen">
      <header className="topbar">
        <button type="button" className="btn ghost" onClick={() => void go({ name: "library" })}>
          <Icon name="back" />
          Library
        </button>
        <h1 className="h-inline">Design with AI</h1>
        <div className="grow" />
        <button
          type="button"
          className="btn ghost"
          disabled={req === EMPTY}
          onClick={async () => {
            if (described && !(await ui.confirm("Start over?", "The description, photos and chosen skills are cleared.", "Start over"))) return;
            setReq(EMPTY);
          }}
        >
          <Icon name="restore" size={18} />
          Start over
        </button>
      </header>
      <main className="ask">
        <p className="lead">Describe what you need, add photos and pick the skills it needs. Then hand it to your AI: it writes the design, and you fine-tune it here with sliders.</p>

        <div className="ask-grid">
          <section className="card-panel ask-what" aria-labelledby="ask-what">
            <h2 id="ask-what">1 · What should it be?</h2>
            <label className="field">
              <span>Name (optional)</span>
              <input value={req.title} maxLength={80} placeholder="e.g. Headphone hook" onChange={(e) => update({ title: e.target.value })} />
            </label>
            <label className="field">
              <span>Description</span>
              <textarea
                className="prose"
                value={req.brief}
                placeholder={"In your own words: what it's for, what it holds or fits, the sizes you know, where it goes and how it mounts.\n\nFor example: “A hook for two pairs of headphones that clamps under my 25 mm desk top, printable without supports.”"}
                onChange={(e) => update({ brief: e.target.value })}
              />
            </label>

            <div className="field">
              <span>Photos (optional)</span>
              <div
                className={`photo-drop ${dragging ? "on" : ""}`}
                onDragOver={(e) => {
                  e.preventDefault();
                  setDragging(true);
                }}
                onDragLeave={() => setDragging(false)}
                onDrop={(e) => {
                  e.preventDefault();
                  setDragging(false);
                  void addFiles(Array.from(e.dataTransfer.files));
                }}
              >
                <Icon name="image" size={26} />
                <div>
                  <button type="button" className="link" onClick={() => fileInput.current?.click()}>
                    Choose photos
                  </button>
                  , drop them here, or paste one with Ctrl+V.
                  <div className="muted small">Put a ruler, a coin or a credit card next to the object so sizes can be measured.</div>
                </div>
                <input
                  ref={fileInput}
                  type="file"
                  accept="image/*"
                  multiple
                  hidden
                  onChange={(e) => {
                    void addFiles(Array.from(e.target.files ?? []));
                    e.target.value = "";
                  }}
                />
              </div>
              {req.photos.length > 0 && (
                <div className="photo-grid">
                  {req.photos.map((p, i) => (
                    <figure key={p.dataUrl.slice(-40) + i} className="photo-tile">
                      <div className="photo-img">
                        <img src={p.dataUrl} alt={p.caption || `Photo ${i + 1}`} />
                        <span className="photo-num">{i + 1}</span>
                        <div className="photo-actions">
                          <button
                            type="button"
                            className="icon-btn small"
                            aria-label={`Copy photo ${i + 1}`}
                            title="Copy, to paste into a chat"
                            onClick={() =>
                              void copyImage(p.dataUrl).then(
                                () => ui.toast(`Photo ${i + 1} copied`, { kind: "ok" }),
                                (e) => ui.toast(String(e), { kind: "error" }),
                              )
                            }
                          >
                            <Icon name="copy" size={16} />
                          </button>
                          <button type="button" className="icon-btn small" aria-label={`Remove photo ${i + 1}`} title="Remove" onClick={() => update({ photos: req.photos.filter((_, j) => j !== i) })}>
                            <Icon name="close" size={16} />
                          </button>
                        </div>
                      </div>
                      <input
                        aria-label={`What photo ${i + 1} shows`}
                        placeholder="What does it show?"
                        value={p.caption}
                        onChange={(e) => update({ photos: req.photos.map((q, j) => (j === i ? { ...q, caption: e.target.value } : q)) })}
                      />
                    </figure>
                  ))}
                </div>
              )}
            </div>

            <label className="check-row">
              <input type="checkbox" checked={req.withPrinter} onChange={(e) => update({ withPrinter: e.target.checked })} />
              <span>
                Size it for my printer: <b>{printer.id === "custom" ? "Custom printer" : printer.name}</b>{" "}
                <span className="muted">
                  · {fmtSize(printer.bed as [number, number, number], settings.units)} · {printer.nozzle} mm nozzle
                </span>
              </span>
            </label>
          </section>

          <section className="card-panel ask-skills" aria-labelledby="ask-skills">
            <div className="row skills-head">
              <h2 id="ask-skills" className="grow">2 · Skills</h2>
              {desktop && (
                <button type="button" className="btn small ghost" title={skills?.folder ?? undefined} onClick={() => void openSkillsFolder()}>
                  <Icon name="folder" size={16} />
                  Open skills folder
                </button>
              )}
              <MenuButton
                className="icon-btn small ghost"
                ariaLabel="More skill actions"
                align="right"
                button={<Icon name="more" size={16} />}
                items={[
                  { label: "Reload skills", icon: "restore", run: () => void loadSkills(true) },
                  ...(desktop ? [{ label: "Restore built-in skills…", icon: "history", run: () => void restoreSkills() }] : []),
                ]}
              />
            </div>
            <p className="muted">
              Skills teach the AI how to design things for this app. The core skills are always included; tick the ones your design needs.
              {desktop && " Add your own or change these in the skills folder: changes show up here straight away."}
            </p>
            {skills && skills.problems.length > 0 && (
              <div className="skill-problems" role="alert">
                <Icon name="alert" size={17} />
                <div>
                  <b>Some skills need fixing</b>
                  <ul>
                    {skills.problems.map((p) => (
                      <li key={p}>{p}</li>
                    ))}
                  </ul>
                </div>
              </div>
            )}
            {!catalog && <p className="muted">Loading skills…</p>}
            {core.length > 0 && <h3 className="section-title">Always included</h3>}
            {core.map((s) => (
              <div key={s.id} className="skill-row core">
                <span className="skill-check" title="Always included">
                  <Icon name="lock" size={17} />
                </span>
                <SkillThumb skill={s} />
                <div className="skill-text grow">
                  <span className="strong">{capitalize(s.title.replace(/^Core: /, ""))}</span>
                  <span className="muted small">{s.summary}</span>
                </div>
                <button type="button" className="icon-btn small ghost" aria-label={`Export the ${s.title} skill`} title="Export this skill (.zip)" disabled={busy !== null} onClick={() => void exportOne(s)}>
                  <Icon name="download" size={17} />
                </button>
              </div>
            ))}
            {optional.map((s, i) => {
              const firstOfCategory = i === 0 || optional[i - 1].category !== s.category;
              const on = selected.includes(s.id);
              const neededBy = optional.filter((o) => selected.includes(o.id) && o.requires.includes(s.id));
              const grouped = groupSize(s.group) > 1;
              const firstOfGroup = grouped && optional.findIndex((o) => o.group === s.group) === i;
              return (
                <Fragment key={s.id}>
                  {firstOfCategory && <h3 className="section-title">{s.category}</h3>}
                  {firstOfGroup && <h4 className="skill-group">{capitalize(s.group!)} · pick one</h4>}
                  <div className={`skill-row ${on ? "on" : ""} ${grouped ? "in-group" : ""}`}>
                    <input
                      id={`skill-${s.id}`}
                      type="checkbox"
                      className="skill-check"
                      checked={on}
                      onChange={(e) => {
                        if (!catalog) return;
                        if (!e.target.checked && neededBy.length > 0) {
                          ui.toast(`${neededBy.map((o) => o.title).join(" and ")} needs ${s.title}: untick that first.`, { kind: "info" });
                          return;
                        }
                        update({ optional: pick(catalog, req.optional, s.id, e.target.checked) });
                      }}
                    />
                    <SkillThumb skill={s} />
                    <label htmlFor={`skill-${s.id}`} className="skill-text grow">
                      <span className="strong">{grouped ? capitalize(s.title.replace(new RegExp(`^${s.group}: `, "i"), "")) : s.title}</span>
                      <span className="muted small">{s.summary}</span>
                      {(s.requires.length > 0 || (on && neededBy.length > 0)) && (
                        <span className="skill-notes small">
                          {s.requires.length > 0 && <span>Adds {s.requires.map(title).join(", ")}</span>}
                          {on && neededBy.length > 0 && <span>Needed by {neededBy.map((o) => o.title).join(", ")}</span>}
                        </span>
                      )}
                    </label>
                    <button type="button" className="icon-btn small ghost" aria-label={`Export the ${s.title} skill`} title="Export this skill (.zip)" disabled={busy !== null} onClick={() => void exportOne(s)}>
                      <Icon name="download" size={17} />
                    </button>
                  </div>
                </Fragment>
              );
            })}
          </section>

          <section className="card-panel ask-send" aria-labelledby="ask-send">
            <h2 id="ask-send">3 · Send it to your AI</h2>
            <div className="send-grid">
              <div className={`send-card ${desktop ? "recommended" : ""}`}>
                <div className="send-head">
                  <Icon name="sparkle" size={20} className="accent-text" />
                  <h3>Claude, connected to the app</h3>
                  {desktop && <span className="badge accent">Recommended</span>}
                </div>
                <p className="muted small">
                  Claude Code or Claude Desktop with ParametricDesign connected (
                  <button type="button" className="link" onClick={() => void go({ name: "settings", back: { name: "ask" } })}>
                    Settings → Claude
                  </button>
                  ). Claude gets your description, photos, printer and skills, designs it, checks it and puts it in your library.
                </p>
                <button type="button" className="btn primary" disabled={!desktop || busy !== null} onClick={() => void sendToClaude()}>
                  <Icon name="send" size={18} />
                  {busy === "send" ? "Sending…" : "Send to Claude"}
                </button>
                {!desktop && <p className="muted small">Needs the desktop app.</p>}
                {result?.kind === "sent" && (
                  <div className="send-result">
                    <p className="small">
                      <b>Now paste this into Claude</b> (it's copied):
                    </p>
                    <CopyBox text={result.message} />
                  </div>
                )}
                <p className="muted small">To change a design you already have, open it and ask Claude there: you get a side-by-side compare.</p>
              </div>

              <div className="send-card">
                <div className="send-head">
                  <Icon name="box" size={20} />
                  <h3>Apps that use skills</h3>
                </div>
                <p className="muted small">For Claude.ai and Claude Code: one skill made for this design, with the chosen skills, your description and photos, as a .zip.</p>
                <button type="button" className="btn" disabled={busy !== null} onClick={() => void exportSkill()}>
                  <Icon name="download" size={18} />
                  {busy === "skill" ? "Exporting…" : "Export tailored skill…"}
                </button>
                {!described && <p className="muted small">Without a description you get a general skill with the skills you chose.</p>}
                {result?.kind === "skill" && (
                  <div className="send-result">
                    <p className="small">
                      Saved <b>{fileName(result.path)}</b>. Install it: in Claude.ai, <b>Settings → Capabilities → Skills → Upload skill</b>; for Claude Code, unzip it into the <code>.claude\skills</code> folder in your user folder. Then start a new chat{photosNote} and send:
                    </p>
                    <CopyBox text={result.starter} />
                  </div>
                )}
              </div>

              <div className="send-card">
                <div className="send-head">
                  <Icon name="file" size={20} />
                  <h3>Any AI chat</h3>
                </div>
                <p className="muted small">For ChatGPT, Gemini and others: everything in one prompt to paste, or to attach as a file.</p>
                <div className="row wrap">
                  <button type="button" className="btn" disabled={busy !== null} onClick={() => void copyPrompt()}>
                    <Icon name="copy" size={18} />
                    {busy === "copy" ? "Copying…" : "Copy prompt"}
                  </button>
                  <button type="button" className="btn" disabled={busy !== null} onClick={() => void savePrompt()}>
                    <Icon name="download" size={18} />
                    {busy === "save" ? "Saving…" : "Save as file…"}
                  </button>
                </div>
                {result?.kind === "prompt-copied" && (
                  <p className="send-result small">
                    <b>Copied.</b> Paste it into a new chat{photosNote}{request.photos.length ? " (Copy on each photo, then paste)" : ""} and send it.
                  </p>
                )}
                {result?.kind === "prompt-saved" && (
                  <div className="send-result">
                    <p className="small">
                      Saved <b>{fileName(result.path)}</b>. Attach it to a new chat{photosNote} and send:
                    </p>
                    <CopyBox text={result.starter} />
                  </div>
                )}
                <p className="muted small">When it answers with the design JSON, add it with New design → Paste JSON.</p>
              </div>
            </div>
          </section>
        </div>
      </main>
    </div>
  );
}

/** The skill's catalog picture, if it has one. */
function SkillThumb({ skill }: { skill: SkillInfo }) {
  if (!skill.thumbnail) return null;
  return <img className="skill-thumb" src={skill.thumbnail} alt="" />;
}

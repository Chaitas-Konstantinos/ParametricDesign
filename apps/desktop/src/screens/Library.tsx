import { useEffect, useMemo, useRef, useState } from "react";
import { useApp } from "../App";
import type { Collection, Summary } from "../api";
import { Icon } from "../components/Icon";
import { MenuButton, useUi, type MenuItem } from "../components/ui";
import { fmtSize, timeAgo } from "../settings";
import { renderThumbnail } from "../thumbnail";

type View = { kind: "all" | "favorites" | "recent" | "claude" | "trash" } | { kind: "collection"; id: string } | { kind: "category"; name: string };

const CATEGORY_COLORS = ["#3b82f6", "#22a35a", "#a855f7", "#ea7a1a", "#0ea5b7", "#e11d74", "#ca8a04", "#64748b"];

export function categoryOf(d: { category: string }): string {
  const top = (d.category || "").split("/")[0].trim();
  if (!top) return "Other";
  return top.replace(/[-_]/g, " ").replace(/\b\w/g, (c) => c.toUpperCase()).replace(/ And /g, " & ");
}

function TagEditor({ initialTags, initialCategory, onChange }: { initialTags: string[]; initialCategory: string; onChange: (tags: string, category: string) => void }) {
  const [tags, setTags] = useState(initialTags.join(", "));
  const [cat, setCat] = useState(initialCategory);
  useEffect(() => onChange(tags, cat), [tags, cat]);
  return (
    <div className="stack">
      <label className="field">
        <span>Category (e.g. storage/boxes)</span>
        <input value={cat} onChange={(e) => setCat(e.target.value)} />
      </label>
      <label className="field">
        <span>Tags, separated by commas</span>
        <input value={tags} onChange={(e) => setTags(e.target.value)} autoFocus />
      </label>
    </div>
  );
}

function CollectionPicker({ collections, initial, onChange }: { collections: Collection[]; initial: string[]; onChange: (ids: string[]) => void }) {
  const [sel, setSel] = useState<string[]>(initial);
  useEffect(() => onChange(sel), [sel]);
  if (!collections.length) return <p className="muted">No collections yet. Create one with the + next to “Collections”.</p>;
  return (
    <div className="stack">
      {collections.map((c) => (
        <label key={c.id} className="check-row">
          <input type="checkbox" checked={sel.includes(c.id)} onChange={(e) => setSel((s) => (e.target.checked ? [...s, c.id] : s.filter((x) => x !== c.id)))} />
          {c.name}
        </label>
      ))}
    </div>
  );
}

export function LibraryScreen() {
  const { api, settings, go, libraryRev, bumpLibrary } = useApp();
  const ui = useUi();
  const [items, setItems] = useState<Summary[] | null>(null);
  const [collections, setCollections] = useState<Collection[]>([]);
  const [view, setView] = useState<View>({ kind: "all" });
  const [query, setQuery] = useState("");
  const [tagFilter, setTagFilter] = useState<string | null>(null);
  const [sort, setSort] = useState<"modified" | "name" | "created" | "opened">("modified");
  const [error, setError] = useState<string | null>(null);
  const search = useRef<HTMLInputElement>(null);
  const thumbsBusy = useRef(false);

  const reload = async () => {
    try {
      const [list, cols] = await Promise.all([api.list(), api.collections()]);
      setItems(list);
      setCollections(cols);
      setError(null);
    } catch (e) {
      setError(String(e));
    }
  };
  useEffect(() => {
    void reload();
  }, [libraryRev]);

  // thumbnails for designs that have none yet (new from Claude, imports)
  useEffect(() => {
    if (!items || thumbsBusy.current) return;
    const todo = items.filter((d) => !d.thumbnail && !d.trashed);
    if (!todo.length) return;
    thumbsBusy.current = true;
    (async () => {
      for (const d of todo) {
        try {
          const v = await api.readVersion(d.id);
          const ev = await api.evaluate(JSON.stringify(v.design), v.params ?? {}, {});
          const t = renderThumbnail(ev.meshes);
          if (t) {
            await api.setThumbnail(d.id, t.png);
            await api.updateMeta(d.id, { info: t.info });
            setItems((list) => list?.map((x) => (x.id === d.id ? { ...x, thumbnail: t.png, info: t.info } : x)) ?? list);
          }
        } catch {
          // a broken design keeps its placeholder
        }
      }
      thumbsBusy.current = false;
    })();
  }, [items]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "f") {
        e.preventDefault();
        search.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const live = (items ?? []).filter((d) => !d.trashed);
  const trashed = (items ?? []).filter((d) => d.trashed);
  const fromClaude = live.filter((d) => d.isNew);
  const recentCut = Date.now() - 14 * 24 * 3600 * 1000;
  const recent = live.filter((d) => Math.max(d.opened ?? 0, d.modified) > recentCut);
  const categories = useMemo(() => {
    const m = new Map<string, number>();
    live.forEach((d) => m.set(categoryOf(d), (m.get(categoryOf(d)) ?? 0) + 1));
    return [...m.entries()].sort((a, b) => b[1] - a[1]);
  }, [items]);
  const tags = useMemo(() => {
    const m = new Map<string, number>();
    live.forEach((d) => d.tags.forEach((t) => m.set(t, (m.get(t) ?? 0) + 1)));
    return [...m.entries()].sort((a, b) => b[1] - a[1]).slice(0, 14).map(([t]) => t);
  }, [items]);

  let shown: Summary[];
  let title: string;
  switch (view.kind) {
    case "favorites":
      shown = live.filter((d) => d.favorite);
      title = "Favorites";
      break;
    case "recent":
      shown = recent;
      title = "Recent";
      break;
    case "claude":
      shown = fromClaude;
      title = "New from Claude";
      break;
    case "trash":
      shown = trashed;
      title = "Trash";
      break;
    case "collection":
      shown = live.filter((d) => d.collections.includes(view.id));
      title = collections.find((c) => c.id === view.id)?.name ?? "Collection";
      break;
    case "category":
      shown = live.filter((d) => categoryOf(d) === view.name);
      title = view.name;
      break;
    default:
      shown = live;
      title = "All designs";
  }
  if (tagFilter) shown = shown.filter((d) => d.tags.includes(tagFilter));
  const q = query.trim().toLowerCase();
  if (q) shown = shown.filter((d) => [d.name, d.description, d.category, ...d.tags].join(" ").toLowerCase().includes(q));
  shown = [...shown].sort((a, b) =>
    sort === "name" ? a.name.localeCompare(b.name) : sort === "created" ? b.created - a.created : sort === "opened" ? (b.opened ?? 0) - (a.opened ?? 0) : b.modified - a.modified,
  );

  const update = async (id: string, patch: Partial<Summary>) => {
    await api.updateMeta(id, patch);
    await reload();
  };

  const newCollection = async () => {
    const name = await ui.prompt("New collection", "Name", "", "Create");
    if (!name) return;
    const id = `${name.toLowerCase().replace(/[^a-z0-9]+/g, "-")}-${Date.now().toString(36)}`;
    const next = [...collections, { id, name }];
    await api.setCollections(next);
    setCollections(next);
    setView({ kind: "collection", id });
  };

  const collectionMenu = (c: Collection): MenuItem[] => [
    {
      label: "Rename",
      icon: "edit",
      run: async () => {
        const name = await ui.prompt("Rename collection", "Name", c.name);
        if (!name) return;
        const next = collections.map((x) => (x.id === c.id ? { ...x, name } : x));
        await api.setCollections(next);
        setCollections(next);
      },
    },
    {
      label: "Delete collection",
      icon: "trash",
      danger: true,
      run: async () => {
        if (!(await ui.confirm(`Delete “${c.name}”?`, "The designs in it stay in your library.", "Delete", true))) return;
        const next = collections.filter((x) => x.id !== c.id);
        await api.setCollections(next);
        for (const d of live.filter((d) => d.collections.includes(c.id))) await api.updateMeta(d.id, { collections: d.collections.filter((x) => x !== c.id) });
        setCollections(next);
        if (view.kind === "collection" && view.id === c.id) setView({ kind: "all" });
        await reload();
      },
    },
  ];

  const cardMenu = (d: Summary): MenuItem[] =>
    d.trashed
      ? [
          { label: "Restore", icon: "restore", run: () => void update(d.id, { trashed: null }) },
          {
            label: "Delete forever",
            icon: "trash",
            danger: true,
            run: async () => {
              if (await ui.confirm(`Delete “${d.name}” forever?`, "All its versions and presets are removed. This cannot be undone.", "Delete forever", true)) {
                await api.deleteForever(d.id);
                await reload();
              }
            },
          },
        ]
      : [
          { label: "Open", icon: "box", run: () => void go({ name: "editor", id: d.id }) },
          { label: d.favorite ? "Remove from favorites" : "Add to favorites", icon: "star", run: () => void update(d.id, { favorite: !d.favorite }) },
          { divider: true, label: "" },
          {
            label: "Rename",
            icon: "edit",
            run: async () => {
              const name = await ui.prompt("Rename design", "Name", d.name);
              if (name) await update(d.id, { name });
            },
          },
          {
            label: "Duplicate",
            icon: "copy",
            run: async () => {
              const id = await api.duplicate(d.id, null, `${d.name} copy`);
              await reload();
              ui.toast(`Duplicated as “${d.name} copy”`, { kind: "ok", action: { label: "Open", run: () => void go({ name: "editor", id }) } });
            },
          },
          {
            label: "Collections…",
            icon: "folder",
            run: async () => {
              let sel = d.collections;
              const r = await ui.dialog({
                title: `Collections for “${d.name}”`,
                body: <CollectionPicker collections={collections} initial={d.collections} onChange={(s) => (sel = s)} />,
                actions: [{ label: "Cancel", value: "cancel" }, { label: "Save", value: "ok", kind: "primary" }],
              });
              if (r?.action === "ok") await update(d.id, { collections: sel });
            },
          },
          {
            label: "Tags & category…",
            icon: "tag",
            run: async () => {
              let tagsText = d.tags.join(", ");
              let cat = d.category;
              const r = await ui.dialog({
                title: `Tags for “${d.name}”`,
                body: <TagEditor initialTags={d.tags} initialCategory={d.category} onChange={(t, c) => ((tagsText = t), (cat = c))} />,
                actions: [{ label: "Cancel", value: "cancel" }, { label: "Save", value: "ok", kind: "primary" }],
              });
              if (r?.action === "ok")
                await update(d.id, {
                  tags: tagsText.split(",").map((t) => t.trim()).filter(Boolean),
                  category: cat.trim(),
                });
            },
          },
          { divider: true, label: "" },
          { label: "Move to trash", icon: "trash", danger: true, run: () => void update(d.id, { trashed: Date.now() }) },
        ];

  const pasteDesign = async () => {
    const r = await ui.dialog({
      title: "Paste a design",
      body: <p className="muted">Paste the JSON your LLM wrote (pd/0.1). If it has problems, the editor lists them with fixes you can send back.</p>,
      textarea: { label: "Design JSON", value: "", placeholder: '{"format": "pd/0.1", ...}' },
      actions: [{ label: "Cancel", value: "cancel" }, { label: "Add to library", value: "ok", kind: "primary" }],
    });
    if (r?.action !== "ok" || !r.text.trim()) return;
    const m = r.text.match(/```(?:json)?\s*([\s\S]*?)```/);
    const text = (m ? m[1] : r.text).trim();
    try {
      JSON.parse(text);
    } catch (e) {
      ui.toast(`That is not valid JSON: ${e}`, { kind: "error" });
      return;
    }
    const id = await api.create(text, {}, "you", "Pasted design");
    bumpLibrary();
    void go({ name: "editor", id });
  };

  const designWithAI = () => void go({ name: "ask" });

  const navItems: { v: View; label: string; icon: string; count: number }[] = [
    { v: { kind: "all" }, label: "All designs", icon: "layers", count: live.length },
    { v: { kind: "favorites" }, label: "Favorites", icon: "star", count: live.filter((d) => d.favorite).length },
    { v: { kind: "recent" }, label: "Recent", icon: "clock", count: recent.length },
    { v: { kind: "claude" }, label: "New from Claude", icon: "sparkle", count: fromClaude.length },
    { v: { kind: "trash" }, label: "Trash", icon: "trash", count: trashed.length },
  ];
  const isView = (v: View) => JSON.stringify(v) === JSON.stringify(view);

  return (
    <div className="screen">
      <header className="topbar">
        <div className="brand">
          <span className="brand-mark">
            <Icon name="logo" size={22} />
          </span>
          ParametricDesign
        </div>
        <div className="search">
          <Icon name="search" size={19} />
          <input ref={search} type="search" aria-label="Search designs" placeholder="Search names, tags, categories… (Ctrl+F)" value={query} onChange={(e) => setQuery(e.target.value)} />
        </div>
        <div className="grow" />
        {fromClaude.length > 0 && (
          <button type="button" className="btn" onClick={() => setView({ kind: "claude" })}>
            <Icon name="sparkle" size={18} className="accent-text" />
            {fromClaude.length} new from Claude
          </button>
        )}
        <MenuButton
          className="btn primary"
          align="right"
          button={
            <>
              <Icon name="plus" size={18} />
              New design
              <Icon name="chevronDown" size={16} />
            </>
          }
          items={[
            { label: "Design with AI…", icon: "sparkle", run: designWithAI },
            { label: "Paste JSON…", icon: "paste", run: pasteDesign },
            {
              label: "Import .pd.json / .3mf…",
              icon: "upload",
              run: async () => {
                try {
                  const ids = await api.importFiles();
                  if (ids.length) {
                    bumpLibrary();
                    ui.toast(`Imported ${ids.length} design${ids.length > 1 ? "s" : ""}`, { kind: "ok" });
                  }
                } catch (e) {
                  ui.toast(String(e), { kind: "error" });
                }
              },
            },
            {
              label: "Add example designs",
              icon: "box",
              run: async () => {
                const n = await api.addExamples();
                bumpLibrary();
                ui.toast(`Added ${n} example designs`, { kind: "ok" });
              },
            },
          ]}
        />
        <button type="button" className="icon-btn" aria-label="Settings" title="Settings" onClick={() => void go({ name: "settings", back: { name: "library" } })}>
          <Icon name="settings" />
        </button>
      </header>

      <div className="body">
        <nav className="sidebar" aria-label="Library">
          <div className="nav-list">
            {navItems.map((n) => (
              <button key={n.label} type="button" className={`nav-item ${isView(n.v) ? "on" : ""}`} onClick={() => setView(n.v)}>
                <Icon name={n.icon} size={19} />
                <span className="grow">{n.label}</span>
                <span className="count">{n.count}</span>
              </button>
            ))}
          </div>

          <div className="nav-list">
            <div className="nav-heading">
              <span>Collections</span>
              <button type="button" className="icon-btn small" aria-label="New collection" title="New collection" onClick={newCollection}>
                <Icon name="plus" size={16} />
              </button>
            </div>
            {collections.length === 0 && <p className="nav-empty">Group designs, e.g. “Desk setup”.</p>}
            {collections.map((c) => (
              <div key={c.id} className={`nav-item with-menu ${isView({ kind: "collection", id: c.id }) ? "on" : ""}`}>
                <button type="button" className="nav-main" onClick={() => setView({ kind: "collection", id: c.id })}>
                  <Icon name="folder" size={18} />
                  <span className="grow">{c.name}</span>
                  <span className="count">{live.filter((d) => d.collections.includes(c.id)).length}</span>
                </button>
                <MenuButton className="icon-btn small ghost" ariaLabel={`${c.name} options`} button={<Icon name="more" size={16} />} items={collectionMenu(c)} align="right" />
              </div>
            ))}
          </div>

          {categories.length > 0 && (
            <div className="nav-list">
              <div className="nav-heading">
                <span>Categories</span>
              </div>
              {categories.map(([name, n], i) => (
                <button key={name} type="button" className={`nav-item ${isView({ kind: "category", name }) ? "on" : ""}`} onClick={() => setView({ kind: "category", name })}>
                  <span className="dot" style={{ background: CATEGORY_COLORS[i % CATEGORY_COLORS.length] }} />
                  <span className="grow">{name}</span>
                  <span className="count">{n}</span>
                </button>
              ))}
            </div>
          )}

          {tags.length > 0 && (
            <div className="nav-list">
              <div className="nav-heading">
                <span>Tags</span>
              </div>
              <div className="chips">
                {tags.map((t) => (
                  <button key={t} type="button" className={`chip ${tagFilter === t ? "on" : ""}`} aria-pressed={tagFilter === t} onClick={() => setTagFilter(tagFilter === t ? null : t)}>
                    {t}
                  </button>
                ))}
              </div>
            </div>
          )}
        </nav>

        <main className="content">
          <div className="content-head">
            <div className="grow">
              <h1>{title}</h1>
              <p className="muted">
                {view.kind === "trash"
                  ? "Designs in the trash can be restored or deleted forever."
                  : `${shown.length} design${shown.length === 1 ? "" : "s"}${tagFilter ? ` tagged “${tagFilter}”` : ""}${q ? ` matching “${query}”` : ""}`}
              </p>
            </div>
            {view.kind === "trash" && trashed.length > 0 && (
              <button
                type="button"
                className="btn danger-outline"
                onClick={async () => {
                  if (!(await ui.confirm("Empty the trash?", `${trashed.length} design(s) are deleted forever.`, "Empty trash", true))) return;
                  for (const d of trashed) await api.deleteForever(d.id);
                  await reload();
                }}
              >
                Empty trash
              </button>
            )}
            <label className="inline-field">
              <span>Sort</span>
              <select value={sort} onChange={(e) => setSort(e.target.value as typeof sort)}>
                <option value="modified">Last changed</option>
                <option value="opened">Last opened</option>
                <option value="created">Newest</option>
                <option value="name">Name</option>
              </select>
            </label>
          </div>

          {error && <div className="banner error">{error}</div>}
          {view.kind === "all" && fromClaude.length > 0 && (
            <button type="button" className="banner accent" onClick={() => setView({ kind: "claude" })}>
              <Icon name="sparkle" size={22} />
              <span className="grow">
                <b>
                  {fromClaude.length === 1 ? `Claude created “${fromClaude[0].name}”` : `${fromClaude.length} new designs from Claude`}
                </b>{" "}
                <span className="muted">· {timeAgo(Math.max(...fromClaude.map((d) => d.created)))}</span>
              </span>
              <span className="accent-text strong">Show</span>
            </button>
          )}

          {items && shown.length === 0 && (
            <div className="empty">
              <Icon name={view.kind === "trash" ? "trash" : "box"} size={40} />
              {live.length === 0 && view.kind === "all" ? (
                <>
                  <h2>Your library is empty</h2>
                  <p className="muted">Describe a design for your AI, paste one, or start from the examples.</p>
                  <div className="row">
                    <button type="button" className="btn" onClick={designWithAI}>
                      <Icon name="sparkle" size={18} /> Design with AI
                    </button>
                    <button
                      type="button"
                      className="btn primary"
                      onClick={async () => {
                        await api.addExamples();
                        bumpLibrary();
                      }}
                    >
                      Add example designs
                    </button>
                  </div>
                </>
              ) : (
                <p className="muted">Nothing here.</p>
              )}
            </div>
          )}

          <div className="cards">
            {shown.map((d) => (
              <div
                key={d.id}
                className="card"
                role="link"
                tabIndex={0}
                aria-label={`Open ${d.name}`}
                onClick={() => !d.trashed && void go({ name: "editor", id: d.id })}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && !d.trashed) void go({ name: "editor", id: d.id });
                }}
              >
                <div className="card-thumb">
                  {d.thumbnail ? <img src={d.thumbnail} alt="" /> : <div className="thumb-placeholder">Preparing preview…</div>}
                  <div className="card-badges">
                    {d.isNew && <span className="badge accent">New from Claude</span>}
                    {d.pendingCount > 0 && <span className="badge warn">Update from Claude</span>}
                  </div>
                  {!d.trashed && (
                    <button
                      type="button"
                      className={`star ${d.favorite ? "on" : ""}`}
                      aria-label={d.favorite ? "Remove from favorites" : "Add to favorites"}
                      aria-pressed={d.favorite}
                      onClick={(e) => {
                        e.stopPropagation();
                        void update(d.id, { favorite: !d.favorite });
                      }}
                    >
                      <Icon name="star" size={22} filled={d.favorite} />
                    </button>
                  )}
                </div>
                <div className="card-body">
                  <div className="card-title-row">
                    <span className="card-title">{d.name}</span>
                    <MenuButton className="icon-btn small ghost" ariaLabel={`${d.name} options`} button={<Icon name="more" size={18} />} items={cardMenu(d)} align="right" />
                  </div>
                  <span className="card-meta">
                    {categoryOf(d)} · v{d.currentVersion}
                    {d.info?.parts ? ` · ${d.info.parts} part${d.info.parts > 1 ? "s" : ""}` : ""}
                    {d.presetCount ? ` · ${d.presetCount} preset${d.presetCount > 1 ? "s" : ""}` : ""}
                  </span>
                  <div className="card-foot">
                    <span>{d.trashed ? `Deleted ${timeAgo(d.trashed)}` : `${d.isNew ? "Arrived" : "Changed"} ${timeAgo(d.modified)}`}</span>
                    <span>{fmtSize(d.info?.size, settings.units)}</span>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </main>
      </div>

      <footer className="statusbar">
        <span title={settings.libraryPath}>Library: {settings.libraryPath}</span>
        <span>
          Printer: {settings.printer.name} · {settings.printer.nozzle} mm
        </span>
        <span className="grow" />
        <span>{api.kind === "desktop" ? "Designs from Claude arrive automatically" : "Browser mode"}</span>
      </footer>
    </div>
  );
}

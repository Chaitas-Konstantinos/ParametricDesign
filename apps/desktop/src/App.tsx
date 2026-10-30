import { createContext, useCallback, useContext, useEffect, useMemo, useRef, useState } from "react";
import { loadApi, type Api, type InboxEvent } from "./api";
import { applyTheme, palette, type Palette } from "./theme";
import { withDefaults, type Settings } from "./settings";
import { UiProvider, useUi } from "./components/ui";
import type { ViewerColors } from "./components/Viewer";
import { LibraryScreen } from "./screens/Library";
import { EditorScreen } from "./screens/Editor";
import { SettingsScreen } from "./screens/Settings";

export type Route = { name: "loading" } | { name: "welcome" } | { name: "library" } | { name: "editor"; id: string; review?: string } | { name: "settings"; back: Route };

interface AppState {
  api: Api;
  settings: Settings;
  pal: Palette;
  colors: ViewerColors;
  saveSettings(s: Settings): Promise<void>;
  go(r: Route): Promise<void>;
  route: Route;
  /** Bumps when the library changed outside the current screen (e.g. Claude). */
  libraryRev: number;
  bumpLibrary(): void;
  lastInbox: { rev: number; events: InboxEvent[] };
  /** The editor registers this: resolves true when it is OK to leave. */
  leaveGuard: React.MutableRefObject<(() => Promise<boolean>) | null>;
}

const Ctx = createContext<AppState | null>(null);

export function useApp(): AppState {
  const a = useContext(Ctx);
  if (!a) throw new Error("app context missing");
  return a;
}

/** Effect cleanup for a listener that is registered asynchronously: if the
 *  effect is torn down before registration finishes, unregister right after. */
function unsubscribeLater(registering: Promise<() => void>) {
  let off: (() => void) | undefined;
  let gone = false;
  registering.then((f) => (gone ? f() : (off = f)));
  return () => {
    gone = true;
    off?.();
  };
}

function Shell({ api }: { api: Api }) {
  const ui = useUi();
  const [settings, setSettings] = useState<Settings | null>(null);
  const [route, setRoute] = useState<Route>({ name: "loading" });
  const [systemTick, setSystemTick] = useState(0);
  const [libraryRev, setLibraryRev] = useState(0);
  const [lastInbox, setLastInbox] = useState<{ rev: number; events: InboxEvent[] }>({ rev: 0, events: [] });
  const leaveGuard = useRef<(() => Promise<boolean>) | null>(null);
  const routeRef = useRef(route);
  routeRef.current = route;

  useEffect(() => {
    api.getSettings().then((raw) => {
      const s = withDefaults(raw);
      setSettings(s);
      setRoute(s.libraryPath ? { name: "library" } : { name: "welcome" });
    });
  }, [api]);

  useEffect(() => {
    const mq = window.matchMedia?.("(prefers-color-scheme: dark)");
    const on = () => setSystemTick((t) => t + 1);
    mq?.addEventListener("change", on);
    return () => mq?.removeEventListener("change", on);
  }, []);

  const pal = useMemo(() => (settings ? applyTheme(settings) : palette(withDefaults({}))), [settings, systemTick]);
  const colors = useMemo<ViewerColors>(
    () => ({ viewer: pal.viewer, plate: pal.plate, grid: pal.grid, gridMajor: pal.gridMajor, accent: pal.accent, accentText: pal.accentText, text: pal.text, muted: pal.muted, danger: pal.dark ? "#e0524f" : "#d93a36" }),
    [pal],
  );

  const go = useCallback(async (r: Route) => {
    const cur = routeRef.current;
    if (cur.name === "editor" && leaveGuard.current && !(r.name === "editor" && r.id === cur.id)) {
      if (!(await leaveGuard.current())) return;
    }
    if (r.name !== "editor") void api.setCurrent({ open: false });
    setRoute(r);
  }, [api]);

  // designs and updates from Claude
  useEffect(
    () =>
      unsubscribeLater(
        api.onInbox((events) => {
          setLibraryRev((n) => n + 1);
          setLastInbox((li) => ({ rev: li.rev + 1, events }));
          for (const ev of events) {
            const cur = routeRef.current;
            if (ev.kind === "update" && cur.name === "editor" && cur.id === ev.id) continue; // the editor opens the compare itself
            if (ev.kind === "new") {
              ui.toast(`Claude created “${ev.name}”`, { kind: "info", action: { label: "Open", run: () => void go({ name: "editor", id: ev.id }) } });
            } else {
              ui.toast(`Claude sent an update to “${ev.name}”`, { kind: "info", action: { label: "Review", run: () => void go({ name: "editor", id: ev.id, review: "latest" }) } });
            }
          }
        }),
      ),
    [api, go],
  );

  // closing the window with unsaved changes
  useEffect(() => unsubscribeLater(api.onCloseRequested(async () => (leaveGuard.current ? leaveGuard.current() : true))), [api]);

  if (!settings) return <div className="splash">Starting…</div>;

  const state: AppState = {
    api,
    settings,
    pal,
    colors,
    async saveSettings(s) {
      await api.saveSettings(s);
      setSettings({ ...s });
    },
    go,
    route,
    libraryRev,
    bumpLibrary: () => setLibraryRev((n) => n + 1),
    lastInbox,
    leaveGuard,
  };

  return (
    <Ctx.Provider value={state}>
      {route.name === "loading" && <div className="splash">Loading…</div>}
      {route.name === "welcome" && <SettingsScreen welcome />}
      {route.name === "library" && <LibraryScreen />}
      {route.name === "editor" && <EditorScreen key={route.id} id={route.id} review={route.review} />}
      {route.name === "settings" && <SettingsScreen back={route.back} />}
    </Ctx.Provider>
  );
}

export function App() {
  const [api, setApi] = useState<Api | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    loadApi().then(setApi, (e) => setError(String(e)));
  }, []);
  if (error) return <div className="splash error">Could not start the engine: {error}</div>;
  if (!api) return <div className="splash">Starting…</div>;
  return (
    <UiProvider>
      <Shell api={api} />
    </UiProvider>
  );
}

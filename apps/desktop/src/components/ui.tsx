// Dialogs, menus and toasts used across the app.
import { createContext, useCallback, useContext, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { Icon } from "./Icon";

export interface DialogAction {
  label: string;
  value: string;
  kind?: "primary" | "danger" | "plain";
}

interface DialogReq {
  title: string;
  body?: React.ReactNode;
  input?: { label: string; value: string; placeholder?: string };
  textarea?: { label: string; value: string; placeholder?: string };
  actions: DialogAction[];
  resolve: (r: { action: string; text: string } | null) => void;
}

interface Toast {
  id: number;
  text: string;
  kind?: "ok" | "error" | "info";
  action?: { label: string; run: () => void };
}

interface Ui {
  dialog(d: Omit<DialogReq, "resolve">): Promise<{ action: string; text: string } | null>;
  confirm(title: string, body: React.ReactNode, ok: string, danger?: boolean): Promise<boolean>;
  prompt(title: string, label: string, value?: string, ok?: string): Promise<string | null>;
  toast(text: string, opts?: { kind?: Toast["kind"]; action?: Toast["action"]; ms?: number }): void;
}

const Ctx = createContext<Ui | null>(null);

export function useUi(): Ui {
  const u = useContext(Ctx);
  if (!u) throw new Error("UiProvider missing");
  return u;
}

export function UiProvider({ children }: { children: React.ReactNode }) {
  const [dlg, setDlg] = useState<DialogReq | null>(null);
  const [text, setText] = useState("");
  const [toasts, setToasts] = useState<Toast[]>([]);
  const nextId = useRef(1);

  const dialog = useCallback((d: Omit<DialogReq, "resolve">) => {
    return new Promise<{ action: string; text: string } | null>((resolve) => {
      setText(d.input?.value ?? d.textarea?.value ?? "");
      setDlg({ ...d, resolve });
    });
  }, []);
  const ui: Ui = {
    dialog,
    async confirm(title, body, ok, danger) {
      const r = await dialog({ title, body, actions: [{ label: "Cancel", value: "cancel" }, { label: ok, value: "ok", kind: danger ? "danger" : "primary" }] });
      return r?.action === "ok";
    },
    async prompt(title, label, value = "", ok = "Save") {
      const r = await dialog({ title, input: { label, value }, actions: [{ label: "Cancel", value: "cancel" }, { label: ok, value: "ok", kind: "primary" }] });
      return r?.action === "ok" && r.text.trim() ? r.text.trim() : null;
    },
    toast(t, opts) {
      const id = nextId.current++;
      setToasts((ts) => [...ts, { id, text: t, kind: opts?.kind, action: opts?.action }]);
      setTimeout(() => setToasts((ts) => ts.filter((x) => x.id !== id)), opts?.ms ?? (opts?.action ? 9000 : 4500));
    },
  };
  const close = (action: string | null) => {
    if (!dlg) return;
    dlg.resolve(action === null ? null : { action, text });
    setDlg(null);
  };
  useEffect(() => {
    if (!dlg) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") close(null);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
  const primary = dlg?.actions.find((a) => a.kind === "primary")?.value;

  return (
    <Ctx.Provider value={ui}>
      {children}
      {dlg && (
        <div className="modal" onMouseDown={() => close(null)}>
          <div className="dialog" role="dialog" aria-modal="true" aria-label={dlg.title} onMouseDown={(e) => e.stopPropagation()}>
            <h2>{dlg.title}</h2>
            {dlg.body && <div className="dialog-body">{dlg.body}</div>}
            {dlg.input && (
              <label className="field">
                <span>{dlg.input.label}</span>
                <input
                  autoFocus
                  value={text}
                  placeholder={dlg.input.placeholder}
                  onChange={(e) => setText(e.target.value)}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && primary) close(primary);
                  }}
                />
              </label>
            )}
            {dlg.textarea && (
              <label className="field">
                <span>{dlg.textarea.label}</span>
                <textarea autoFocus value={text} placeholder={dlg.textarea.placeholder} onChange={(e) => setText(e.target.value)} />
              </label>
            )}
            <div className="dialog-actions">
              {dlg.actions.map((a) => (
                <button key={a.value} type="button" className={`btn ${a.kind === "primary" ? "primary" : a.kind === "danger" ? "danger" : ""}`} onClick={() => close(a.value)}>
                  {a.label}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
      <div className="toasts" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.kind ?? ""}`}>
            <Icon name={t.kind === "error" ? "alert" : t.kind === "ok" ? "check" : "info"} size={18} />
            <span>{t.text}</span>
            {t.action && (
              <button
                type="button"
                className="link"
                onClick={() => {
                  t.action!.run();
                  setToasts((ts) => ts.filter((x) => x.id !== t.id));
                }}
              >
                {t.action.label}
              </button>
            )}
          </div>
        ))}
      </div>
    </Ctx.Provider>
  );
}

export interface MenuItem {
  label: string;
  icon?: string;
  run?: () => void;
  danger?: boolean;
  disabled?: boolean;
  checked?: boolean;
  divider?: boolean;
  hint?: string;
}

/** A button that opens a dropdown menu. The menu floats above everything
 *  (cards and scroll areas don't cut it off): below the button, or above it
 *  when there is no room, kept inside the window. */
export function MenuButton({
  button,
  items,
  align = "left",
  className,
  ariaLabel,
  title,
  up,
  onOpen,
  menuClassName,
}: {
  button: React.ReactNode;
  items: MenuItem[];
  align?: "left" | "right";
  className?: string;
  ariaLabel?: string;
  title?: string;
  up?: boolean;
  onOpen?: () => void;
  menuClassName?: string;
}) {
  const [open, setOpen] = useState(false);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);
  const btn = useRef<HTMLButtonElement>(null);
  const menu = useRef<HTMLDivElement>(null);

  // below the button, or above it when there is no room; inside the window
  const place = useCallback(() => {
    if (!btn.current || !menu.current) return;
    const b = btn.current.getBoundingClientRect();
    if (b.bottom < 0 || b.top > window.innerHeight) {
      setOpen(false); // its button scrolled away
      return;
    }
    const m = menu.current.getBoundingClientRect();
    const gap = 6;
    const pad = 8;
    const left = Math.max(pad, Math.min(align === "right" ? b.right - m.width : b.left, window.innerWidth - m.width - pad));
    const below = b.bottom + gap;
    const above = b.top - gap - m.height;
    const fitsBelow = below + m.height <= window.innerHeight - pad;
    const fitsAbove = above >= pad;
    const top = (up ? fitsAbove || !fitsBelow : !fitsBelow && fitsAbove) ? above : below;
    setPos({ top: Math.max(pad, Math.min(top, window.innerHeight - m.height - pad)), left });
  }, [align, up]);

  useLayoutEffect(() => {
    if (open) place();
  }, [open, items.length, place]);

  useEffect(() => {
    if (!open) return;
    const inside = (t: EventTarget | null) => !!t && (btn.current?.contains(t as Node) || menu.current?.contains(t as Node));
    const off = (e: MouseEvent) => {
      if (!inside(e.target)) setOpen(false);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    // follow the button while the page scrolls
    let frame = 0;
    const follow = (e: Event) => {
      if (menu.current?.contains(e.target as Node)) return;
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(place);
    };
    const resized = () => setOpen(false);
    document.addEventListener("mousedown", off);
    document.addEventListener("keydown", key);
    window.addEventListener("scroll", follow, true);
    window.addEventListener("resize", resized);
    return () => {
      cancelAnimationFrame(frame);
      document.removeEventListener("mousedown", off);
      document.removeEventListener("keydown", key);
      window.removeEventListener("scroll", follow, true);
      window.removeEventListener("resize", resized);
    };
  }, [open, place]);

  return (
    <div className="menu-anchor">
      <button
        ref={btn}
        type="button"
        className={className ?? "btn"}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={ariaLabel}
        title={title}
        onClick={(e) => {
          e.stopPropagation();
          if (!open) {
            setPos(null);
            onOpen?.();
          }
          setOpen((o) => !o);
        }}
        onKeyDown={(e) => {
          // Enter/Space press this button only, not a clickable card around it
          if (e.key === "Enter" || e.key === " ") e.stopPropagation();
        }}
      >
        {button}
      </button>
      {open &&
        createPortal(
          <div
            ref={menu}
            className={`menu floating ${menuClassName ?? ""}`}
            role="menu"
            style={pos ? { top: pos.top, left: pos.left } : { top: 0, left: 0, visibility: "hidden" }}
            onClick={(e) => e.stopPropagation()}
            onMouseDown={(e) => e.stopPropagation()}
            onKeyDown={(e) => {
              if (e.key === "Escape") setOpen(false);
              e.stopPropagation();
            }}
          >
            {items.map((it, i) =>
              it.divider ? (
                <div key={i} className="menu-divider" />
              ) : (
                <button
                  key={i}
                  type="button"
                  role="menuitem"
                  className={`menu-item ${it.danger ? "danger" : ""}`}
                  disabled={it.disabled}
                  onClick={() => {
                    setOpen(false);
                    it.run?.();
                  }}
                >
                  <span className="menu-icon">{it.checked !== undefined ? it.checked ? <Icon name="check" size={16} /> : null : it.icon ? <Icon name={it.icon} size={17} /> : null}</span>
                  <span className="menu-label">{it.label}</span>
                  {it.hint && <span className="menu-hint">{it.hint}</span>}
                </button>
              ),
            )}
          </div>,
          document.body,
        )}
    </div>
  );
}

/** Segmented control. */
export function Segmented<T extends string>({ value, options, onChange, label }: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label: string }) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {options.map((o) => (
        <button key={o.value} type="button" className={o.value === value ? "on" : ""} aria-pressed={o.value === value} onClick={() => onChange(o.value)}>
          {o.label}
        </button>
      ))}
    </div>
  );
}

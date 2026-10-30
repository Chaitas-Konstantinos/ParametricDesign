// Dialogs, menus and toasts used across the app.
import { createContext, useCallback, useContext, useEffect, useRef, useState } from "react";
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

/** A button that opens a dropdown menu. */
export function MenuButton({ button, items, align = "left", className, ariaLabel, title, up }: { button: React.ReactNode; items: MenuItem[]; align?: "left" | "right"; className?: string; ariaLabel?: string; title?: string; up?: boolean }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const off = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", off);
    document.addEventListener("keydown", key);
    return () => {
      document.removeEventListener("mousedown", off);
      document.removeEventListener("keydown", key);
    };
  }, [open]);
  return (
    <div className="menu-anchor" ref={ref}>
      <button
        type="button"
        className={className ?? "btn"}
        aria-haspopup="menu"
        aria-expanded={open}
        aria-label={ariaLabel}
        title={title}
        onClick={(e) => {
          e.stopPropagation();
          setOpen((o) => !o);
        }}
      >
        {button}
      </button>
      {open && (
        <div className={`menu ${align} ${up ? "up" : ""}`} role="menu" onClick={(e) => e.stopPropagation()}>
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
        </div>
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

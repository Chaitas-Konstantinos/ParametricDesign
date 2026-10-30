import type { ParamInfo, ParamValue, Params } from "./engine";

interface Props {
  params: ParamInfo[];
  values: Params;
  advanced: boolean;
  onChange: (name: string, value: ParamValue) => void;
  onReset: () => void;
}

function fmt(v: number): string {
  return Number.isInteger(v) ? String(v) : String(Math.round(v * 1000) / 1000);
}

function Control({ p, value, onChange }: { p: ParamInfo; value: ParamValue; onChange: (v: ParamValue) => void }) {
  switch (p.type) {
    case "boolean":
      return <input type="checkbox" checked={Boolean(value)} onChange={(e) => onChange(e.target.checked)} />;
    case "choice":
      return (
        <select value={String(value)} onChange={(e) => onChange(p.options?.find((o) => String(o) === e.target.value) ?? e.target.value)}>
          {p.options?.map((o) => (
            <option key={String(o)} value={String(o)}>
              {String(o)}
            </option>
          ))}
        </select>
      );
    case "color":
      return <input type="color" value={String(value)} onChange={(e) => onChange(e.target.value)} />;
    default: {
      const min = p.min ?? 0;
      const max = p.max ?? Math.max(1, Number(p.default) * 3);
      const step = p.step ?? (p.type === "integer" ? 1 : (max - min) / 200);
      const n = Number(value);
      return (
        <div className="num">
          <input type="range" min={min} max={max} step={step} value={n} onChange={(e) => onChange(Number(e.target.value))} />
          <input
            type="number"
            min={min}
            max={max}
            step={step}
            value={fmt(n)}
            onChange={(e) => {
              const v = Number(e.target.value);
              if (Number.isFinite(v)) onChange(Math.min(max, Math.max(min, v)));
            }}
          />
          {p.unit && <span className="unit">{p.unit}</span>}
        </div>
      );
    }
  }
}

export function ParamPanel({ params, values, advanced, onChange, onReset }: Props) {
  const shown = params.filter((p) => advanced || !p.advanced);
  const groups = new Map<string, ParamInfo[]>();
  for (const p of shown) {
    const g = p.group ?? "Parameters";
    groups.set(g, [...(groups.get(g) ?? []), p]);
  }
  const hidden = params.length - shown.length;
  return (
    <div className="params">
      {[...groups.entries()].map(([g, ps]) => (
        <fieldset key={g}>
          <legend>{g}</legend>
          {ps.map((p) => (
            <label key={p.name} className={p.advanced ? "param adv" : "param"} title={p.description ?? p.name}>
              <span className="pname">{p.label ?? p.name}</span>
              <Control p={p} value={values[p.name] ?? p.value} onChange={(v) => onChange(p.name, v)} />
            </label>
          ))}
        </fieldset>
      ))}
      <div className="param-foot">
        {hidden > 0 && <span className="muted">{hidden} advanced setting{hidden > 1 ? "s" : ""} hidden</span>}
        <button className="link" onClick={onReset}>
          Reset to defaults
        </button>
      </div>
    </div>
  );
}

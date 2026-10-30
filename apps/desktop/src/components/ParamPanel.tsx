import type { ParamInfo, ParamValue, Params } from "../api";
import { fmtNum, type Units } from "../settings";

interface Props {
  params: ParamInfo[];
  values: Params;
  saved: Params;
  advanced: boolean;
  units: Units;
  onChange: (name: string, value: ParamValue) => void;
}

const isLength = (p: ParamInfo) => p.unit === "mm";

function NumberControl({ p, value, units, onChange, part }: { p: ParamInfo; value: number; units: Units; onChange: (v: number) => void; part: "value" | "slider" }) {
  const inch = units === "in" && isLength(p);
  const k = inch ? 25.4 : 1;
  const min = p.min ?? 0;
  const max = p.max ?? Math.max(1, Number(p.default) * 3);
  const step = p.step ?? (p.type === "integer" ? 1 : (max - min) / 200);
  const shown = inch ? fmtNum(value / k, 3) : fmtNum(value, step < 1 ? 3 : 2);
  const unit = inch ? "in" : p.unit;
  if (part === "value")
    return (
      <div className="param-value">
        <input
          type="number"
          aria-label={`${p.label ?? p.name} value`}
          value={shown}
          step={inch ? Math.max(0.001, step / k) : step}
          min={min / k}
          max={max / k}
          onChange={(e) => {
            const v = Number(e.target.value) * k;
            if (Number.isFinite(v)) onChange(Math.min(max, Math.max(min, p.type === "integer" ? Math.round(v) : v)));
          }}
        />
        {unit && <span className="unit">{unit}</span>}
      </div>
    );
  return (
    <input
        className="param-slider"
        type="range"
        id={`p-${p.name}`}
        min={min}
        max={max}
        step={step}
        value={value}
        onChange={(e) => onChange(Number(e.target.value))}
        style={{ ["--fill" as string]: `${((value - min) / (max - min || 1)) * 100}%` }}
      />
  );
}

export function ParamPanel({ params, values, saved, advanced, units, onChange }: Props) {
  const shown = params.filter((p) => advanced || !p.advanced);
  const groups = new Map<string, ParamInfo[]>();
  for (const p of shown) {
    const g = p.group ?? "Parameters";
    groups.set(g, [...(groups.get(g) ?? []), p]);
  }
  return (
    <div className="params">
      {[...groups.entries()].map(([g, ps]) => (
        <section key={g} className="param-group">
          <h3 className="section-title">{g}</h3>
          {ps.map((p) => {
            const value = values[p.name] ?? p.value;
            const changed = saved[p.name] !== undefined ? saved[p.name] !== value : p.default !== value;
            const numeric = p.type === "number" || p.type === "integer";
            return (
              <div key={p.name} className={`param ${numeric ? "numeric" : ""}`} title={p.description}>
                <div className="param-head">
                  <label htmlFor={`p-${p.name}`} className="param-label">
                    {p.label ?? p.name}
                    {p.advanced && <span className="adv-tag">advanced</span>}
                  </label>
                  {changed && <span className="changed-dot" title="Changed since the last save" />}
                  {p.type === "boolean" && (
                    <button type="button" id={`p-${p.name}`} role="switch" aria-checked={Boolean(value)} className={`switch ${value ? "on" : ""}`} onClick={() => onChange(p.name, !value)}>
                      <span />
                    </button>
                  )}
                  {p.type === "choice" && (
                    <select id={`p-${p.name}`} value={String(value)} onChange={(e) => onChange(p.name, p.options?.find((o) => String(o) === e.target.value) ?? e.target.value)}>
                      {p.options?.map((o) => (
                        <option key={String(o)} value={String(o)}>
                          {String(o)}
                        </option>
                      ))}
                    </select>
                  )}
                  {p.type === "color" && <input id={`p-${p.name}`} type="color" value={String(value)} onChange={(e) => onChange(p.name, e.target.value)} />}
                  {numeric && <NumberControl part="value" p={p} value={Number(value)} units={units} onChange={(v) => onChange(p.name, v)} />}
                </div>
                {numeric && <NumberControl part="slider" p={p} value={Number(value)} units={units} onChange={(v) => onChange(p.name, v)} />}
                {p.description && <p className="param-desc">{p.description}</p>}
              </div>
            );
          })}
        </section>
      ))}
    </div>
  );
}

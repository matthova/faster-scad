// The customizer panel: renders a control per parameter, grouped, and reports
// changes as override values. Untouched params show their source default.
import { useRef, useState } from "react";
import type { Param, ParamValue } from "./customizer";

interface Props {
  params: Param[];
  /** User-set overrides (only touched params appear here). */
  overrides: Record<string, ParamValue>;
  onChange: (name: string, value: ParamValue) => void;
  onReset: () => void;
  /** Saved parameter-set names (presets). */
  presets: string[];
  onApplyPreset: (name: string) => void;
  onSavePreset: () => void;
  onDeletePreset: (name: string) => void;
  onImportPresets: (file: File) => void;
  onExportPresets: () => void;
}

export function CustomizerPanel({
  params,
  overrides,
  onChange,
  onReset,
  presets,
  onApplyPreset,
  onSavePreset,
  onDeletePreset,
  onImportPresets,
  onExportPresets,
}: Props) {
  const [selected, setSelected] = useState("");
  const fileInput = useRef<HTMLInputElement>(null);
  if (params.length === 0) return null;

  // Group params in first-seen order.
  const groups: { name: string; items: Param[] }[] = [];
  for (const p of params) {
    let g = groups.find((x) => x.name === p.group);
    if (!g) {
      g = { name: p.group, items: [] };
      groups.push(g);
    }
    g.items.push(p);
  }

  const dirty = Object.keys(overrides).length > 0;

  return (
    <aside className="params">
      <div className="params-head">
        <span>Parameters</span>
        <button
          onClick={onReset}
          disabled={!dirty}
          title="Reset to source defaults"
        >
          Reset
        </button>
      </div>
      <div className="params-presets">
        <select
          aria-label="Parameter set"
          value={selected}
          onChange={(e) => {
            const name = e.target.value;
            setSelected(name);
            if (name) onApplyPreset(name);
          }}
        >
          <option value="">Preset…</option>
          {presets.map((n) => (
            <option key={n} value={n}>
              {n}
            </option>
          ))}
        </select>
        <button
          onClick={onSavePreset}
          title="Save current values as a named set"
        >
          Save
        </button>
        <button
          onClick={() => {
            if (selected) onDeletePreset(selected);
            setSelected("");
          }}
          disabled={!selected}
          title="Delete the selected set"
        >
          Delete
        </button>
        <button
          onClick={() => fileInput.current?.click()}
          title="Import an OpenSCAD .json set file"
        >
          Import
        </button>
        <button
          onClick={onExportPresets}
          disabled={presets.length === 0}
          title="Export sets as .json"
        >
          Export
        </button>
        <input
          ref={fileInput}
          type="file"
          accept=".json,application/json"
          style={{ display: "none" }}
          onChange={(e) => {
            const f = e.target.files?.[0];
            if (f) onImportPresets(f);
            e.target.value = ""; // allow re-importing the same file
          }}
        />
      </div>
      <div className="params-body">
        {groups.map((g) => (
          <fieldset className="param-group" key={g.name || "_global"}>
            {g.name && <legend>{g.name}</legend>}
            {g.items.map((p) => (
              <Row
                key={p.name}
                param={p}
                value={overrides[p.name] ?? p.value}
                onChange={(v) => onChange(p.name, v)}
              />
            ))}
          </fieldset>
        ))}
      </div>
    </aside>
  );
}

/** Round a slider value for display so digit count stays bounded (mid-drag the
 *  raw float would otherwise vary in width). Precision follows step when given,
 *  else caps at 3 decimals; trailing zeros are dropped. */
function formatSlider(value: number, step: number | null): string {
  if (!Number.isFinite(value)) return String(value);
  let decimals = 3;
  if (step && step > 0) {
    const dot = String(step).indexOf(".");
    decimals = dot === -1 ? 0 : String(step).length - dot - 1;
  }
  return String(Number(value.toFixed(decimals)));
}

function Row({
  param,
  value,
  onChange,
}: {
  param: Param;
  value: ParamValue;
  onChange: (v: ParamValue) => void;
}) {
  const label = param.description || param.name;
  const c = param.control;

  return (
    <label className="param-row" title={param.name}>
      <span className="param-label">{label}</span>
      {c.kind === "checkbox" && (
        <input
          type="checkbox"
          checked={Boolean(value)}
          onChange={(e) => onChange(e.target.checked)}
        />
      )}

      {c.kind === "slider" && (
        <span className="param-slider">
          <input
            type="range"
            min={c.min}
            max={c.max}
            step={c.step ?? "any"}
            value={Number(value)}
            onChange={(e) => onChange(parseFloat(e.target.value))}
          />
          <output>{formatSlider(Number(value), c.step)}</output>
        </span>
      )}

      {c.kind === "number" && (
        <input
          type="number"
          value={Number(value)}
          onChange={(e) =>
            onChange(e.target.value === "" ? 0 : parseFloat(e.target.value))
          }
        />
      )}

      {c.kind === "text" && (
        <input
          type="text"
          maxLength={c.maxLength ?? undefined}
          value={String(value)}
          onChange={(e) => onChange(e.target.value)}
        />
      )}

      {c.kind === "dropdown" && (
        <select
          value={String(value)}
          onChange={(e) => {
            const opt = c.options.find(
              (o) => String(o.value) === e.target.value,
            );
            onChange(opt ? opt.value : e.target.value);
          }}
        >
          {c.options.map((o, i) => (
            <option key={i} value={String(o.value)}>
              {o.label}
            </option>
          ))}
        </select>
      )}

      {c.kind === "vector" && (
        <span className="param-vector">
          {Array.from({ length: c.length }).map((_, i) => {
            const arr = Array.isArray(value) ? value : [];
            return (
              <input
                key={i}
                type="number"
                value={Number(arr[i] ?? 0)}
                onChange={(e) => {
                  const next = [...(Array.isArray(value) ? value : [])];
                  while (next.length < c.length) next.push(0);
                  next[i] =
                    e.target.value === "" ? 0 : parseFloat(e.target.value);
                  onChange(next);
                }}
              />
            );
          })}
        </span>
      )}
    </label>
  );
}

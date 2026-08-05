// The right dock: one scrollable column of collapsible sections (Parameters,
// then Model) — not exclusive tabs, because you read the Model numbers *while*
// dragging a Parameter. When a script has no params the whole dock collapses to
// a ~28px labelled spine so it stays discoverable without costing 288px.
import type { ReactNode } from "react";
import { CustomizerPanel } from "./CustomizerPanel";
import type { Param, ParamValue } from "./customizer";
import type { PreviewGroup, MeshInfo } from "./viewer";

/** The Model-section data (a subset of the render status). */
export interface ModelInfo {
  ok: boolean;
  triangleCount: number;
  vertexCount: number;
  area: number;
  volume: number;
  preview: boolean;
  geomErrors: string;
  dims: MeshInfo | null;
  /** Colored preview groups (empty for single-color models). */
  groups: PreviewGroup[];
  /** Names of the resolved in-project library files (tabs beyond main). */
  libraries: string[];
}

interface DockProps {
  // Customizer
  params: Param[];
  overrides: Record<string, ParamValue>;
  onChange: (name: string, value: ParamValue) => void;
  onReset: () => void;
  presets: string[];
  onApplyPreset: (name: string) => void;
  onSavePreset: () => void;
  onDeletePreset: (name: string) => void;
  onImportPresets: (file: File) => void;
  onExportPresets: () => void;
  // Model
  model: ModelInfo;
  // Layout
  collapsed: boolean;
  onToggleCollapsed: () => void;
  paramsOpen: boolean;
  onToggleParams: () => void;
  modelOpen: boolean;
  onToggleModel: () => void;
}

function Section({
  title,
  open,
  onToggle,
  children,
}: {
  title: string;
  open: boolean;
  onToggle: () => void;
  children: ReactNode;
}) {
  return (
    <section className={`dock-section ${open ? "" : "collapsed"}`}>
      <button
        className="dock-section-head"
        onClick={onToggle}
        aria-expanded={open}
      >
        <span className="dock-caret">{open ? "▾" : "▸"}</span>
        <span className="dock-section-title">{title}</span>
      </button>
      {open && <div className="dock-section-body">{children}</div>}
    </section>
  );
}

const int = (n: number) => n.toLocaleString();
const f2 = (n: number) =>
  n.toLocaleString(undefined, {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
const cssColor = (c: [number, number, number, number]) =>
  `rgba(${Math.round(c[0] * 255)}, ${Math.round(c[1] * 255)}, ${Math.round(
    c[2] * 255,
  )}, ${c[3]})`;

function ModelPanel({ model }: { model: ModelInfo }) {
  if (!model.ok) return <div className="dock-empty">No model yet.</div>;
  const { dims, groups } = model;
  const parts = groups.filter((g) => g.mode !== "background");
  return (
    <div className="model-panel">
      <dl className="model-stats">
        <dt>Triangles</dt>
        <dd>{int(model.triangleCount)}</dd>
        <dt>Vertices</dt>
        <dd>{int(model.vertexCount)}</dd>
        <dt>Area</dt>
        <dd>{f2(model.area)}</dd>
        <dt>Volume</dt>
        <dd>{model.preview ? `≈ ${f2(model.volume)}` : f2(model.volume)}</dd>
        {dims && (
          <>
            <dt>Size</dt>
            <dd>
              {fmt(dims.x)} × {fmt(dims.y)} × {fmt(dims.z)} mm
            </dd>
          </>
        )}
        <dt>Integrity</dt>
        <dd
          className={
            model.geomErrors
              ? "integrity-degraded"
              : model.preview
                ? "integrity-preview"
                : "integrity-exact"
          }
        >
          {model.geomErrors
            ? "Degraded — a CSG op failed"
            : model.preview
              ? "Fast preview — not watertight"
              : "Exact — watertight"}
        </dd>
      </dl>

      {parts.length > 1 && (
        <div className="model-parts">
          <div className="model-subhead">{parts.length} colored parts</div>
          <ul>
            {parts.map((g, i) => (
              <li key={i}>
                <span
                  className="part-swatch"
                  style={{ background: cssColor(g.color) }}
                />
                <span className="part-count">{int(g.count / 3)} tris</span>
                {g.mode === "highlight" && (
                  <span className="part-mode"># highlight</span>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}

      {model.libraries.length > 0 && (
        <div className="model-libs">
          <div className="model-subhead">Libraries</div>
          <ul>
            {model.libraries.map((name) => (
              <li key={name}>{name}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

/** Format a bounding-box dimension: whole numbers plain, else up to 2 decimals. */
function fmt(v: number): string {
  return Number.isInteger(v) ? String(v) : v.toFixed(2).replace(/\.?0+$/, "");
}

export function Dock(props: DockProps) {
  if (props.collapsed) {
    return (
      <aside className="dock collapsed">
        <button
          className="dock-spine"
          onClick={props.onToggleCollapsed}
          title="Show the Parameters and Model panels"
          aria-label="Expand dock"
        >
          <span className="dock-spine-label">Parameters · Model</span>
        </button>
      </aside>
    );
  }
  const hasParams = props.params.length > 0;
  return (
    <aside className="dock">
      <button
        className="dock-collapse"
        onClick={props.onToggleCollapsed}
        title="Collapse the dock to a spine"
        aria-label="Collapse dock"
      >
        ⟩
      </button>
      <Section
        title="Parameters"
        open={props.paramsOpen}
        onToggle={props.onToggleParams}
      >
        {hasParams ? (
          <CustomizerPanel
            params={props.params}
            overrides={props.overrides}
            onChange={props.onChange}
            onReset={props.onReset}
            presets={props.presets}
            onApplyPreset={props.onApplyPreset}
            onSavePreset={props.onSavePreset}
            onDeletePreset={props.onDeletePreset}
            onImportPresets={props.onImportPresets}
            onExportPresets={props.onExportPresets}
          />
        ) : (
          <div className="dock-empty">This script has no parameters.</div>
        )}
      </Section>
      <Section
        title="Model"
        open={props.modelOpen}
        onToggle={props.onToggleModel}
      >
        <ModelPanel model={props.model} />
      </Section>
    </aside>
  );
}

// Gemeinsame Seitenleiste der Visualisierungsfenster (UI-090): Suche, Konfiguration (Knoten, Beschriftung, Linien,
// Farben, Skalierung), Knotenbudget (UI-096), Legende mit Skala (UI-095), Export (SVG, PNG 1×/2×, CSV).
import { ChevronDown, ChevronUp } from "lucide-react";
import { useEffect, useId, useState, type ReactNode } from "react";
import { getFormatters, useT } from "../../i18n";
import { Banner, Button, Checkbox, IconButton, RadioGroup, SearchField, Switch } from "../../ui/primitives";
import { metricOptions } from "./model/details";
import { PALETTES, type ColorMode, type EncodeStats, type Palette } from "./model/encoding";
import type { Scene } from "./model/scene";
import type { SearchState } from "./model/search";
import { DEFAULT_BUDGET, MAX_BUDGET, clampBudget, type VizOptions } from "./options";
import { layoutOf, type VizWindowKind } from "./open";
import type { LayoutState } from "./GraphView";

export type ExportState = { kind: "idle" } | { kind: "running" } | { kind: "done"; path: string } | { kind: "failed"; message: string };

export interface SidebarProps {
  kind: VizWindowKind;
  options: VizOptions;
  onOptions: (patch: Partial<VizOptions>) => void;
  availableMetrics: readonly string[];
  stats: EncodeStats | null;
  scene: Scene | null;
  search: SearchState;
  onSearch: (query: string) => void;
  onStep: (dir: 1 | -1) => void;
  onExpandDepth: () => void;
  layoutState: LayoutState;
  onPause: () => void;
  onResume: () => void;
  exportState: ExportState;
  onExportSvg: () => void;
  onExportPng: (factor: 1 | 2) => void;
  onExportCsv: () => void;
  csvBlocked: boolean;
  /** `null` in der App (nativer Ordnerdialog), sonst Textfeld. */
  csvDir: string | null;
  onCsvDir: (dir: string) => void;
  hasData: boolean;
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  const id = useId();
  return (
    <section className="os-viz-section" aria-labelledby={id}>
      <h2 id={id} className="os-viz-section-title">
        {title}
      </h2>
      {children}
    </section>
  );
}

/** Zahlenfeld, das erst beim Verlassen oder mit Enter übernimmt (vermeidet Neuladen bei jedem Tastendruck). */
export function NumberField({ label, value, onCommit, min, max, step = 1 }: { label: string; value: number; onCommit: (v: number) => void; min?: number; max?: number; step?: number }) {
  const id = useId();
  const [text, setText] = useState(String(value));
  useEffect(() => setText(String(value)), [value]);
  const commit = () => {
    const v = Number(text.replace(",", "."));
    if (!Number.isFinite(v)) return setText(String(value));
    const clamped = Math.min(max ?? Infinity, Math.max(min ?? -Infinity, v));
    if (clamped !== value) onCommit(clamped);
    else setText(String(value));
  };
  return (
    <div className="os-viz-field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        className="os-input"
        type="number"
        inputMode="decimal"
        value={text}
        min={min}
        max={max}
        step={step}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
        }}
      />
    </div>
  );
}

function SelectField({ label, value, onChange, options }: { label: string; value: string; onChange: (v: string) => void; options: readonly { value: string; label: string; disabled?: boolean; title?: string }[] }) {
  const id = useId();
  return (
    <div className="os-viz-field">
      <label htmlFor={id}>{label}</label>
      <select id={id} className="os-input" value={value} onChange={(e) => onChange(e.target.value)}>
        {options.map((o) => (
          <option key={o.value} value={o.value} disabled={o.disabled} title={o.title}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function Swatch({ color, hollow, dashed, label }: { color: string; hollow?: boolean; dashed?: boolean; label: string }) {
  return (
    <li className="os-viz-legend-item">
      <svg width="14" height="14" aria-hidden>
        <circle cx="7" cy="7" r="5.5" fill={hollow ? "none" : color} stroke={hollow || dashed ? (dashed ? "#5f6670" : color) : "none"} strokeWidth="1.5" strokeDasharray={dashed ? "2 1.5" : undefined} />
      </svg>
      <span>{label}</span>
    </li>
  );
}

export function Legend({ kind, options, stats, scene }: { kind: VizWindowKind; options: VizOptions; stats: EncodeStats | null; scene: Scene | null }) {
  const t = useT();
  const fmt = getFormatters();
  const pal = PALETTES[options.palette];
  const num = (v: number) => (Number.isInteger(v) ? fmt.integer(v) : fmt.decimal(v, 2));
  const segments = new Map<string, string>();
  if (options.colorMode === "segment" && scene) for (const n of scene.nodes) if (!segments.has(n.fill) && !n.capped && !n.hollow) segments.set(n.fill, n.fill);
  const metricName = (key: string) => metricOptions([key]).find((m) => m.key === key)?.name ?? key;
  return (
    <div className="os-viz-legend" data-testid="viz-legend">
      <ul>
        {options.colorMode === "indexability" && (
          <>
            <Swatch color={pal.indexable} label={t("viz-legend-indexable")} />
            <Swatch color={pal.nonIndexable} label={t("viz-legend-non-indexable")} />
          </>
        )}
        {options.colorMode === "statusClass" &&
          (["2xx", "3xx", "4xx", "5xx", "none"] as const).map((c) => <Swatch key={c} color={pal.status[c]} label={t(`viz-legend-status-${c}`)} />)}
        {options.colorMode === "segment" && (
          <>
            {[...segments.keys()].map((c) => <Swatch key={c} color={c} label={c} />)}
            <Swatch color={pal.noData} label={t("viz-legend-no-segment")} />
          </>
        )}
        <Swatch color={pal.capped} label={t("viz-legend-capped")} />
        {kind === "directoryTree" && <Swatch color={pal.path} hollow label={t("viz-legend-path")} />}
        {options.sizeByMetric && <Swatch color={pal.noData} dashed label={t("viz-legend-missing")} />}
      </ul>
      {options.colorMode === "metric" && stats && (
        <div className="os-viz-scale">
          <span className="os-viz-scale-name">{metricName(options.colorMetric)}</span>
          <span className="os-viz-gradient" style={{ background: `linear-gradient(to right, ${pal.gradient[0]}, ${pal.gradient[1]})` }} aria-hidden />
          <span className="os-viz-scale-range">
            <span>{t("viz-legend-min", { value: num(stats.minColor) })}</span>
            <span>{t("viz-legend-max", { value: num(stats.maxColor) })}</span>
          </span>
        </div>
      )}
      {options.sizeByMetric && stats && (
        <p className="os-viz-scale">
          {t("viz-legend-size", { metric: metricName(options.metric), max: num(stats.maxMetric), rmin: options.minRadius, rmax: options.maxRadius })}
        </p>
      )}
      {layoutOf(kind) === "force" && !options.sizeByMetric && <p className="os-viz-scale">{t("viz-legend-depth")}</p>}
    </div>
  );
}

export function Sidebar(p: SidebarProps) {
  const t = useT();
  const o = p.options;
  const force = layoutOf(p.kind) === "force";
  const metrics = metricOptions(p.availableMetrics);
  const metricSelect = metrics.map((m) => ({ value: m.key, label: m.name, disabled: m.disabled, title: m.disabled ? t("viz-metric-unavailable") : undefined }));
  const [expandTo, setExpandTo] = useState(o.expandDepth);
  const counter = p.search.matches.length ? t("viz-search-count", { index: p.search.index + 1, total: p.search.matches.length }) : p.search.query ? t("viz-search-none") : "";

  return (
    <aside className="os-viz-sidebar" aria-label={t("viz-sidebar-label")}>
      <Section title={t("viz-section-search")}>
        <div
          className="os-viz-search"
          onKeyDown={(e) => {
            if (e.key === "Enter" && e.shiftKey) {
              e.preventDefault();
              p.onStep(-1);
            }
          }}
        >
          <SearchField label={t("viz-search-label")} value={p.search.query} onChange={p.onSearch} onSubmit={() => p.onStep(1)} clearLabel={t("viz-search-clear")} placeholder={t("viz-search-placeholder")} />
          <IconButton label={t("viz-search-prev")} icon={<ChevronUp size={14} />} isDisabled={!p.search.matches.length} onPress={() => p.onStep(-1)} />
          <IconButton label={t("viz-search-next")} icon={<ChevronDown size={14} />} isDisabled={!p.search.matches.length} onPress={() => p.onStep(1)} />
        </div>
        <p className="os-viz-search-count" aria-live="polite" data-testid="viz-search-count">
          {counter}
        </p>
      </Section>

      <Section title={t("viz-section-nodes")}>
        {!force && (
          <>
            <RadioGroup
              label={t("viz-orientation")}
              orientation="horizontal"
              value={o.orientation}
              onChange={(v) => p.onOptions({ orientation: v === "tb" ? "tb" : "lr" })}
              options={[
                { value: "lr", label: t("viz-orientation-lr") },
                { value: "tb", label: t("viz-orientation-tb") },
              ]}
            />
            <div className="os-viz-row">
              <NumberField label={t("viz-expand-depth")} value={expandTo} min={0} max={1000} onCommit={setExpandTo} />
              <Button
                onPress={() => {
                  if (expandTo === o.expandDepth) p.onExpandDepth();
                  else p.onOptions({ expandDepth: expandTo });
                }}
              >
                {t("viz-expand-apply")}
              </Button>
            </div>
            <NumberField label={t("viz-level-gap")} value={o.levelGap} min={20} max={1000} onCommit={(v) => p.onOptions({ levelGap: v })} />
            <NumberField label={t("viz-node-gap")} value={o.nodeGap} min={4} max={200} onCommit={(v) => p.onOptions({ nodeGap: v })} />
            <RadioGroup
              label={t("viz-label-mode")}
              orientation="horizontal"
              value={o.labelMode}
              onChange={(v) => p.onOptions({ labelMode: v === "url" ? "url" : "path" })}
              options={[
                { value: "path", label: t("viz-label-path") },
                { value: "url", label: t("viz-label-url") },
              ]}
            />
          </>
        )}
        {force && (
          <>
            <NumberField label={t("viz-r0")} value={o.r0} min={3} max={100} step={0.5} onCommit={(v) => p.onOptions({ r0: v })} />
            <NumberField label={t("viz-falloff")} value={o.f} min={0.1} max={1} step={0.05} onCommit={(v) => p.onOptions({ f: v })} />
            <Switch isSelected={o.allowOverlap} onChange={(v) => p.onOptions({ allowOverlap: v })}>
              {t("viz-allow-overlap")}
            </Switch>
            <NumberField label={t("viz-repulsion")} value={o.repulsion} min={1} max={1000} onCommit={(v) => p.onOptions({ repulsion: v })} />
            <NumberField label={t("viz-spring")} value={o.springLength} min={1} max={500} onCommit={(v) => p.onOptions({ springLength: v })} />
            <NumberField label={t("viz-label-zoom")} value={o.labelZoom} min={0.05} max={8} step={0.1} onCommit={(v) => p.onOptions({ labelZoom: v })} />
            <NumberField label={t("viz-seed")} value={o.seed} min={0} max={2 ** 31 - 1} onCommit={(v) => p.onOptions({ seed: Math.round(v) })} />
            <div className="os-viz-row">
              {p.layoutState.phase === "paused" ? (
                <Button onPress={p.onResume}>{t("viz-resume")}</Button>
              ) : (
                <Button onPress={p.onPause} isDisabled={p.layoutState.phase !== "running"}>
                  {t("viz-pause")}
                </Button>
              )}
              <span className="os-viz-layout-state" data-testid="viz-layout-state" data-phase={p.layoutState.phase}>
                {t(`viz-layout-${p.layoutState.phase}`, { steps: p.layoutState.steps })}
              </span>
            </div>
          </>
        )}
        <NumberField label={t("viz-line-width")} value={o.lineWidth} min={0.25} max={10} step={0.25} onCommit={(v) => p.onOptions({ lineWidth: v })} />
        <Checkbox isSelected={o.includeNonHtml} onChange={(v) => p.onOptions({ includeNonHtml: v })}>
          {t("viz-include-non-html")}
        </Checkbox>
        <Checkbox isSelected={o.includeNonIndexable} onChange={(v) => p.onOptions({ includeNonIndexable: v })}>
          {t("viz-include-non-indexable")}
        </Checkbox>
      </Section>

      <Section title={t("viz-section-colors")}>
        <SelectField
          label={t("viz-color-mode")}
          value={o.colorMode}
          onChange={(v) => p.onOptions({ colorMode: v as ColorMode })}
          options={(["indexability", "statusClass", "segment", "metric"] as const).map((m) => ({ value: m, label: t(`viz-color-${m}`) }))}
        />
        {o.colorMode === "metric" && <SelectField label={t("viz-color-metric")} value={o.colorMetric} onChange={(v) => p.onOptions({ colorMetric: v })} options={metricSelect} />}
        <SelectField
          label={t("viz-palette")}
          value={o.palette}
          onChange={(v) => p.onOptions({ palette: v as Palette })}
          options={[
            { value: "standard", label: t("viz-palette-standard") },
            { value: "colorblind", label: t("viz-palette-colorblind") },
          ]}
        />
        <SelectField label={t("viz-metric")} value={o.metric} onChange={(v) => p.onOptions({ metric: v })} options={metricSelect} />
        <Switch isSelected={o.sizeByMetric} onChange={(v) => p.onOptions({ sizeByMetric: v })}>
          {t("viz-size-by-metric")}
        </Switch>
        {o.sizeByMetric && (
          <div className="os-viz-row">
            <NumberField label={t("viz-min-radius")} value={o.minRadius} min={1} max={o.maxRadius} onCommit={(v) => p.onOptions({ minRadius: v })} />
            <NumberField label={t("viz-max-radius")} value={o.maxRadius} min={o.minRadius} max={200} onCommit={(v) => p.onOptions({ maxRadius: v })} />
          </div>
        )}
      </Section>

      <Section title={t("viz-section-budget")}>
        <NumberField label={t("viz-budget")} value={o.budget} min={1} max={MAX_BUDGET} onCommit={(v) => p.onOptions({ budget: clampBudget(v) })} />
        {o.budget > DEFAULT_BUDGET && <Banner kind="warning">{t("viz-budget-warning")}</Banner>}
      </Section>

      <Section title={t("viz-section-legend")}>
        <Legend kind={p.kind} options={o} stats={p.stats} scene={p.scene} />
      </Section>

      <Section title={t("viz-section-export")}>
        <div className="os-viz-row os-viz-wrap">
          <Button onPress={p.onExportSvg} isDisabled={!p.hasData}>
            {t("viz-export-svg")}
          </Button>
          <Button onPress={() => p.onExportPng(1)} isDisabled={!p.hasData}>
            {t("viz-export-png", { factor: 1 })}
          </Button>
          <Button onPress={() => p.onExportPng(2)} isDisabled={!p.hasData}>
            {t("viz-export-png", { factor: 2 })}
          </Button>
        </div>
        {p.csvDir !== null && (
          <div className="os-viz-field">
            <label htmlFor="os-viz-csv-dir">{t("viz-export-csv-dir")}</label>
            <input id="os-viz-csv-dir" className="os-input" value={p.csvDir} onChange={(e) => p.onCsvDir(e.target.value)} />
          </div>
        )}
        <Button onPress={p.onExportCsv} isDisabled={!p.hasData || p.csvBlocked || p.exportState.kind === "running" || (p.csvDir !== null && !p.csvDir.trim())}>
          {t("viz-export-csv")}
        </Button>
        {p.csvBlocked && <p className="os-viz-hint">{t("viz-export-csv-collapsed")}</p>}
        {p.exportState.kind === "running" && <p role="status">{t("viz-export-running")}</p>}
        {p.exportState.kind === "done" && <p role="status" data-testid="viz-export-done">{t("viz-export-done", { path: p.exportState.path })}</p>}
        {p.exportState.kind === "failed" && <Banner kind="error">{p.exportState.message}</Banner>}
      </Section>
    </aside>
  );
}

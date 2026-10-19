// Inlink-Ankertext-Wortwolke (UI-099): Begriffe aus `viz.words`, Schriftgröße linear 12–64 px, deterministische
// Platzierung, Tabelle `Term`/`Count`; Klick auf einen Begriff zeigt im Hauptfenster die Inlinks gefiltert auf ihn.
import { useEffect, useMemo, useRef, useState } from "react";
import { currentLanguage, getFormatters, useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import type { WordFreq } from "../../ipc/gen/WordFreq";
import { viz } from "../../ipc/viz";
import { Banner, Button, Checkbox, EmptyState, RadioGroup } from "../../ui/primitives";
import { readRenderStyle, saveBlob, svgBlob } from "./exporters";
import { layoutWords, sortTerms, type PlacedWord } from "./model/words";
import { requestInlinksForAnchor } from "./open";
import { NumberField } from "./Sidebar";

export interface WordOptions {
  mode: "phrase" | "word";
  removeStopwords: boolean;
  maxTerms: number;
  includeNofollow: boolean;
}

export const DEFAULT_WORD_OPTIONS: WordOptions = { mode: "phrase", removeStopwords: true, maxTerms: 100, includeNofollow: true };

export interface WordDebug {
  terms: WordFreq[];
  placed: PlacedWord[];
  svg(): string;
}

declare global {
  interface Window {
    __osVizWords?: WordDebug;
  }
}

function fontStack(): string {
  if (typeof document === "undefined") return "sans-serif";
  return getComputedStyle(document.documentElement).getPropertyValue("--os-font-ui").trim() || "sans-serif";
}

export function WordCloudView({ snapshotId, urlId, generation }: { snapshotId: string; urlId: number | null; generation: number }) {
  const t = useT();
  const fmt = getFormatters();
  const [opts, setOpts] = useState<WordOptions>(DEFAULT_WORD_OPTIONS);
  const [terms, setTerms] = useState<WordFreq[] | null>(null);
  const [placed, setPlaced] = useState<PlacedWord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [size, setSize] = useState({ width: 0, height: 0 });
  const stage = useRef<HTMLDivElement>(null);
  const svg = useRef<SVGSVGElement>(null);
  const font = useMemo(fontStack, []);
  const ink = useMemo(() => readRenderStyle().text, []);

  useEffect(() => {
    if (urlId === null) return;
    const ctrl = new AbortController();
    setError(null);
    viz
      .words(
        { snapshotId, kind: "anchors", urlId, mode: opts.mode, removeStopwords: opts.removeStopwords, language: currentLanguage(), maxTerms: opts.maxTerms, includeNofollow: opts.includeNofollow },
        ctrl.signal,
      )
      .then((w) => setTerms(sortTerms(w)))
      .catch((e: unknown) => {
        if (!ctrl.signal.aborted) setError(describeError(t, toCmdError(e)).text);
      });
    return () => ctrl.abort();
  }, [generation, opts, snapshotId, t, urlId]);

  useEffect(() => {
    const el = stage.current;
    if (!el) return;
    const measure = () => {
      const r = el.getBoundingClientRect();
      setSize((s) => (Math.abs(s.width - r.width) < 1 && Math.abs(s.height - r.height) < 1 ? s : { width: r.width, height: r.height }));
    };
    measure();
    const ro = typeof ResizeObserver === "function" ? new ResizeObserver(measure) : null;
    ro?.observe(el);
    return () => ro?.disconnect();
  }, [terms !== null]);

  useEffect(() => {
    if (!terms) return;
    let live = true;
    void layoutWords(terms, size.width || 800, size.height || 500, font).then((p) => live && setPlaced(p));
    return () => {
      live = false;
    };
  }, [font, size.height, size.width, terms]);

  const open = (term: string) => {
    if (urlId === null) return;
    void viz.focusInMain({ urlId }).catch((e: unknown) => setError(describeError(t, toCmdError(e)).text));
    requestInlinksForAnchor(urlId, term);
  };

  const serialize = () => (svg.current ? new XMLSerializer().serializeToString(svg.current) : "");
  useEffect(() => {
    if (!import.meta.env.DEV && import.meta.env.VITE_E2E !== "1") return;
    window.__osVizWords = { terms: terms ?? [], placed, svg: serialize };
  });

  if (urlId === null) return <EmptyState state={{ code: "NO_SELECTION" }} />;
  const w = Math.max(64, Math.floor(size.width || 800));
  const h = Math.max(64, Math.floor(size.height || 500));

  return (
    <div className="os-viz-body">
      <aside className="os-viz-sidebar" aria-label={t("viz-sidebar-label")}>
        <section className="os-viz-section">
          <h2 className="os-viz-section-title">{t("viz-section-words")}</h2>
          <RadioGroup
            label={t("viz-words-unit")}
            orientation="horizontal"
            value={opts.mode}
            onChange={(v) => setOpts((o) => ({ ...o, mode: v === "word" ? "word" : "phrase" }))}
            options={[
              { value: "phrase", label: t("viz-words-phrase") },
              { value: "word", label: t("viz-words-word") },
            ]}
          />
          <Checkbox isSelected={opts.removeStopwords} onChange={(v) => setOpts((o) => ({ ...o, removeStopwords: v }))}>
            {t("viz-words-stopwords")}
          </Checkbox>
          <Checkbox isSelected={opts.includeNofollow} onChange={(v) => setOpts((o) => ({ ...o, includeNofollow: v }))}>
            {t("viz-words-nofollow")}
          </Checkbox>
          <NumberField label={t("viz-words-max")} value={opts.maxTerms} min={1} max={1000} onCommit={(v) => setOpts((o) => ({ ...o, maxTerms: Math.round(v) }))} />
        </section>
        <section className="os-viz-section">
          <h2 className="os-viz-section-title">{t("viz-section-export")}</h2>
          <div className="os-viz-row os-viz-wrap">
            <Button isDisabled={!placed.length} onPress={() => saveBlob("anchor-words.svg", svgBlob(`<?xml version="1.0" encoding="UTF-8"?>\n${serialize()}`))}>
              {t("viz-export-svg")}
            </Button>
          </div>
        </section>
        <section className="os-viz-section">
          <h2 className="os-viz-section-title">{t("viz-section-terms")}</h2>
          <table className="os-viz-terms" data-testid="viz-terms">
            <thead>
              <tr>
                <th scope="col">Term</th>
                <th scope="col">Count</th>
              </tr>
            </thead>
            <tbody>
              {(terms ?? []).map((w) => (
                <tr key={w.word}>
                  <td>
                    <button type="button" className="os-viz-term-link" onClick={() => open(w.word)}>
                      {w.word}
                    </button>
                  </td>
                  <td>{fmt.integer(w.count)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </aside>
      <section className="os-viz-main" aria-label={t("viz-kind-anchorWords")}>
        {error && <Banner kind="error">{error}</Banner>}
        {terms !== null && terms.length === 0 && <EmptyState state={{ code: "NO_DATA_YET" }} />}
        <div ref={stage} className="os-viz-stage">
          {placed.length > 0 && (
            <svg ref={svg} xmlns="http://www.w3.org/2000/svg" version="1.1" width={w} height={h} viewBox={`0 0 ${w} ${h}`} role="group" aria-label={t("viz-kind-anchorWords")}>
              <g transform={`translate(${w / 2} ${h / 2})`}>
                {placed.map((p) => (
                  // Gruppe als Schaltfläche mit expliziter Trefferfläche: Textglyphen allein sind je Engine unterschiedlich klickbar.
                  <g
                    key={p.text}
                    className="os-viz-word"
                    role="button"
                    tabIndex={0}
                    aria-label={t("viz-word-label", { term: p.text, count: p.count })}
                    onClick={() => open(p.text)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        open(p.text);
                      }
                    }}
                  >
                    <rect x={p.x + p.box.x0} y={p.y + p.box.y0} width={p.box.x1 - p.box.x0} height={p.box.y1 - p.box.y0} fill="none" pointerEvents="all" />
                    <text x={p.x} y={p.y} fontSize={p.size} fontFamily={font} fill={ink} textAnchor="middle">
                      {p.text}
                    </text>
                  </g>
                ))}
              </g>
            </svg>
          )}
        </div>
      </section>
    </div>
  );
}

// Wortwolken: Inlink-Ankertexte (UI-099) und sichtbarer Body-Text (UI-100). Begriffe aus `viz.words`, Schriftgröße
// linear 12–64 px, deterministische Platzierung, Tabelle `Term`/`Count`. Ankertexte: Klick auf einen Begriff zeigt im
// Hauptfenster die Inlinks gefiltert auf ihn. Body-Text ohne gespeichertes HTML: Hinweis `HTML_NOT_STORED` (UI-016).
import { useEffect, useMemo, useRef, useState } from "react";
import { currentLanguage, getFormatters, useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import type { WordFreq } from "../../ipc/gen/WordFreq";
import { viz } from "../../ipc/viz";
import type { EmptyState as EmptyStateDto } from "../../ui/emptyState/codes";
import { Banner, Button, Checkbox, EmptyState, RadioGroup } from "../../ui/primitives";
import { readRenderStyle, saveBlob, svgBlob } from "./exporters";
import { fitWords, layoutWords, sortTerms, type PlacedWord, type WordTier } from "./model/words";
import { requestInlinksForAnchor } from "./open";
import { NumberField } from "./Sidebar";

export interface WordOptions {
  mode: "phrase" | "word";
  removeStopwords: boolean;
  maxTerms: number;
  includeNofollow: boolean;
  /** Body-Text (UI-100): Mindestlänge eines Worts. */
  minLength: number;
}

export type WordCloudSource = "anchors" | "body";

export const DEFAULT_WORD_OPTIONS: WordOptions = { mode: "phrase", removeStopwords: true, maxTerms: 100, includeNofollow: true, minLength: 3 };

/** Standardwerte der Body-Wortwolke (UI-100): Stoppwörter an, Mindestlänge 3, max. 100 Begriffe. */
export const DEFAULT_BODY_OPTIONS: WordOptions = { ...DEFAULT_WORD_OPTIONS, mode: "word" };

/** Hinweis nach UI-016 statt einer Fehlermeldung, wenn für die URL kein HTML gespeichert ist. */
export const HTML_NOT_STORED_STATE: EmptyStateDto = { code: "HTML_NOT_STORED", action: { kind: "openSetting", anchor: "extraction.store_html" } };

export interface WordDebug {
  source: WordCloudSource;
  urlId: number | null;
  terms: WordFreq[];
  placed: PlacedWord[];
  empty: string | null;
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

export function WordCloudView({ snapshotId, urlId: givenUrl, generation, source = "anchors" }: { snapshotId: string; urlId: number | null; generation: number; source?: WordCloudSource }) {
  const t = useT();
  const fmt = getFormatters();
  const body = source === "body";
  const kind = body ? "bodyWords" : "anchorWords";
  const [opts, setOpts] = useState<WordOptions>(body ? DEFAULT_BODY_OPTIONS : DEFAULT_WORD_OPTIONS);
  const [terms, setTerms] = useState<WordFreq[] | null>(null);
  const [placed, setPlaced] = useState<PlacedWord[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [empty, setEmpty] = useState<EmptyStateDto | null>(null);
  // Ohne URL (Menü „Visualisierungen“, Wurzel = Start-URL; oder Fenster aus dem Tabellen-Kontextmenü): die Wurzel des
  // Schnappschusses über `viz.graph` bestimmen (erster Knoten des Crawl-Baums = Wurzel beim Öffnen bzw. Start-URL).
  const [resolved, setResolved] = useState<number | null>(givenUrl);
  const [resolving, setResolving] = useState(givenUrl === null);
  useEffect(() => {
    if (givenUrl !== null) {
      setResolved(givenUrl);
      setResolving(false);
      return;
    }
    let live = true;
    setResolving(true);
    viz
      .graph({ snapshotId, kind: "crawlTree", root: null, budget: 1, metric: "crawl_depth", expandDepth: 0 })
      .then((g) => live && setResolved(g.nodes[0]?.id ?? null))
      .catch(() => live && setResolved(null))
      .finally(() => live && setResolving(false));
    return () => {
      live = false;
    };
  }, [givenUrl, snapshotId, generation]);
  const urlId = resolved;
  const [size, setSize] = useState({ width: 0, height: 0 });
  const stage = useRef<HTMLDivElement>(null);
  const svg = useRef<SVGSVGElement>(null);
  const font = useMemo(fontStack, []);
  // Farben nach Häufigkeitsstufe aus den Design-Tokens: Akzent, Text, Erfolgsfarbe, gedämpfter Text.
  const tierColors = useMemo((): Record<WordTier, string> => {
    const st = readRenderStyle();
    return { 0: st.accent, 1: st.text, 2: st.palette?.indexable ?? st.text, 3: st.badgeFill };
  }, []);
  const [hoverWord, setHoverWord] = useState<{ text: string; count: number; x: number; y: number } | null>(null);
  const hoverAt = (p: PlacedWord, e: { clientX: number; clientY: number }) => {
    const r = stage.current?.getBoundingClientRect();
    setHoverWord({ text: p.text, count: p.count, x: e.clientX - (r?.left ?? 0) + 12, y: e.clientY - (r?.top ?? 0) + 12 });
  };

  useEffect(() => {
    if (urlId === null) return;
    const ctrl = new AbortController();
    setError(null);
    setEmpty(null);
    const args = body
      ? { snapshotId, kind: "body" as const, urlId, removeStopwords: opts.removeStopwords, language: currentLanguage(), maxTerms: opts.maxTerms, minLength: opts.minLength }
      : { snapshotId, kind: "anchors" as const, urlId, mode: opts.mode, removeStopwords: opts.removeStopwords, language: currentLanguage(), maxTerms: opts.maxTerms, includeNofollow: opts.includeNofollow };
    viz
      .words(args, ctrl.signal)
      .then((w) => setTerms(sortTerms(w)))
      .catch((e: unknown) => {
        if (ctrl.signal.aborted) return;
        const err = toCmdError(e);
        if (err.messageKey === "err-viz-html-not-stored") {
          setTerms([]);
          setEmpty(HTML_NOT_STORED_STATE);
        } else setError(describeError(t, err).text);
      });
    return () => ctrl.abort();
  }, [body, generation, opts, snapshotId, t, urlId]);

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
    if (urlId === null || body) return;
    void viz.focusInMain({ urlId }).catch((e: unknown) => setError(describeError(t, toCmdError(e)).text));
    requestInlinksForAnchor(urlId, term);
  };

  const serialize = () => (svg.current ? new XMLSerializer().serializeToString(svg.current) : "");
  useEffect(() => {
    if (!import.meta.env.DEV && import.meta.env.VITE_E2E !== "1") return;
    window.__osVizWords = { source, urlId, terms: terms ?? [], placed, empty: empty?.code ?? null, svg: serialize };
  });

  if (urlId === null) return resolving ? null : <EmptyState state={{ code: "NO_SELECTION" }} />;
  const w = Math.max(64, Math.floor(size.width || 800));
  const h = Math.max(64, Math.floor(size.height || 500));
  // Wolke zentriert und flächenfüllend (Darstellung; die Schriftgrößen 12–64 px der Platzierung bleiben).
  const fit = fitWords(placed, w, h);

  return (
    <div className="os-viz-body">
      <aside className="os-viz-sidebar" aria-label={t("viz-sidebar-label")}>
        <section className="os-viz-section">
          <h2 className="os-viz-section-title">{t("viz-section-words")}</h2>
          {!body && (
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
          )}
          <Checkbox isSelected={opts.removeStopwords} onChange={(v) => setOpts((o) => ({ ...o, removeStopwords: v }))}>
            {t("viz-words-stopwords")}
          </Checkbox>
          {!body && (
            <Checkbox isSelected={opts.includeNofollow} onChange={(v) => setOpts((o) => ({ ...o, includeNofollow: v }))}>
              {t("viz-words-nofollow")}
            </Checkbox>
          )}
          {body && <NumberField label={t("viz-words-min-length")} value={opts.minLength} min={1} max={100} onCommit={(v) => setOpts((o) => ({ ...o, minLength: Math.round(v) }))} />}
          <NumberField label={t("viz-words-max")} value={opts.maxTerms} min={1} max={1000} onCommit={(v) => setOpts((o) => ({ ...o, maxTerms: Math.round(v) }))} />
        </section>
        <section className="os-viz-section">
          <h2 className="os-viz-section-title">{t("viz-section-export")}</h2>
          <div className="os-viz-row os-viz-wrap">
            <Button isDisabled={!placed.length} onPress={() => saveBlob(body ? "body-words.svg" : "anchor-words.svg", svgBlob(`<?xml version="1.0" encoding="UTF-8"?>\n${serialize()}`))}>
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
                    {body ? (
                      w.word
                    ) : (
                      <button type="button" className="os-viz-term-link" onClick={() => open(w.word)}>
                        {w.word}
                      </button>
                    )}
                  </td>
                  <td>{fmt.integer(w.count)}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </section>
      </aside>
      <section className="os-viz-main" aria-label={t(`viz-kind-${kind}`)}>
        {error && <Banner kind="error">{error}</Banner>}
        {empty && <EmptyState state={empty} />}
        {!empty && terms !== null && terms.length === 0 && <EmptyState state={{ code: "NO_DATA_YET" }} />}
        <div ref={stage} className="os-viz-stage os-viz-stage-soft">
          {placed.length > 0 && (
            <svg ref={svg} xmlns="http://www.w3.org/2000/svg" version="1.1" width={w} height={h} viewBox={`0 0 ${w} ${h}`} role="group" aria-label={t(`viz-kind-${kind}`)}>
              <g transform={`translate(${w / 2} ${h / 2}) scale(${fit.k.toFixed(4)}) translate(${(-fit.cx).toFixed(2)} ${(-fit.cy).toFixed(2)})`}>
                {body && placed.map((p) => (
                  <text
                    key={p.text}
                    className="os-viz-word-plain"
                    x={p.x}
                    y={p.y}
                    fontSize={p.size}
                    fontWeight={p.weight}
                    fontFamily={font}
                    fill={hoverWord?.text === p.text ? tierColors[0] : tierColors[p.tier]}
                    textAnchor="middle"
                    onMouseMove={(e) => hoverAt(p, e)}
                    onMouseLeave={() => setHoverWord(null)}
                  >
                    <title>{t("viz-body-word-label", { term: p.text, count: p.count })}</title>
                    {p.text}
                  </text>
                ))}
                {!body && placed.map((p) => (
                  // Gruppe als Schaltfläche mit expliziter Trefferfläche: Textglyphen allein sind je Engine unterschiedlich klickbar.
                  <g
                    key={p.text}
                    className="os-viz-word"
                    role="button"
                    tabIndex={0}
                    aria-label={t("viz-word-label", { term: p.text, count: p.count })}
                    onClick={() => open(p.text)}
                    onMouseMove={(e) => hoverAt(p, e)}
                    onMouseLeave={() => setHoverWord(null)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter" || e.key === " ") {
                        e.preventDefault();
                        open(p.text);
                      }
                    }}
                  >
                    <rect x={p.x + p.box.x0} y={p.y + p.box.y0} width={p.box.x1 - p.box.x0} height={p.box.y1 - p.box.y0} fill="none" pointerEvents="all" />
                    <text x={p.x} y={p.y} fontSize={p.size} fontWeight={p.weight} fontFamily={font} fill={hoverWord?.text === p.text ? tierColors[0] : tierColors[p.tier]} textAnchor="middle">
                      {p.text}
                    </text>
                  </g>
                ))}
              </g>
            </svg>
          )}
          {hoverWord && (
            <div className="os-viz-word-tip" role="tooltip" style={{ left: hoverWord.x, top: hoverWord.y }}>
              {t("viz-word-tip", { term: hoverWord.text, count: fmt.integer(hoverWord.count) })}
            </div>
          )}
        </div>
      </section>
    </div>
  );
}

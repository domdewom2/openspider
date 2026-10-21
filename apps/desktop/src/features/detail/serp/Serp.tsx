import { useEffect, useMemo, useRef, useState } from "react";
import { Pencil } from "lucide-react";
import { detail } from "../../../ipc/detail";
import { isSilentError } from "../../../ipc/errors";
import type { SerpData } from "../../../ipc/gen/SerpData";
import type { SerpDevice } from "../../../ipc/gen/SerpDevice";
import type { SerpMeasureRow } from "../../../ipc/gen/SerpMeasureRow";
import { useLanguageStore, useT } from "../../../i18n";
import { Button } from "../../../ui/primitives/Button";
import { Banner } from "../../../ui/primitives/Feedback";
import { RadioGroup, TextField } from "../../../ui/primitives/fields";
import { DetailError, DetailLoading } from "../frame/Feedback";
import { useDetailRequest } from "../frame/request";
import type { DetailPanelProps } from "../frame/types";
import { crawlDate, boldSegments, codepoints, displayText, parseTerms, prefixText, SERP_DEVICES, type SerpPrefixKind } from "./model";
import "./serp.css";

const SAVE_DELAY_MS = 400;
interface Form { key: string; title: string; description: string }

function Rich({ text, terms }: { text: string; terms: readonly string[] }) {
  return <>{boldSegments(text, terms).map((part, index) => part.bold ? <b key={index}>{part.text}</b> : <span key={index}>{part.text}</span>)}</>;
}

export function Serp({ selection, actions }: DetailPanelProps) {
  const t = useT(); const lang = useLanguageStore(s => s.lang);
  const focus = selection.focus ?? selection.urlIds[0]!;
  const urlKey = `${selection.crawlId ?? ""}:${focus}`;
  const [device, setDevice] = useState<SerpDevice>("desktop");
  const [form, setForm] = useState<Form>();
  const [termsInput, setTermsInput] = useState(""); const [prefixKind, setPrefixKind] = useState<SerpPrefixKind>("none");
  const [prefixDate, setPrefixDate] = useState(""); const [prefixCustom, setPrefixCustom] = useState("");
  const [measure, setMeasure] = useState<SerpMeasureRow[]>([]); const [editedAt, setEditedAt] = useState<string>();
  const result = useDetailRequest(`serp:${urlKey}:${device}`, signal => detail.serp({ urlId: focus, device }, signal));
  const data: SerpData | undefined = result.value;
  // Crawl-Datum als Standard des Datumspräfix (UI-037); ohne Wert das heutige Datum.
  const stamp = useDetailRequest(`serp-date:${urlKey}`, async signal => (await detail.urlDetails({ urlId: focus }, signal)).find(row => row.name === "Crawl Timestamp")?.value);
  useEffect(() => { if (stamp.value !== undefined || !stamp.loading) setPrefixDate(crawlDate(stamp.value, new Date())); }, [stamp.value, stamp.loading, urlKey]);
  // Die Eingabe bleibt beim Gerätewechsel erhalten; ein anderer Crawl oder eine andere URL setzt sie neu.
  useEffect(() => {
    if (data && form?.key !== urlKey) {
      setForm({ key: urlKey, title: data.edited?.title ?? data.title, description: data.edited?.description ?? data.description });
      setEditedAt(data.edited?.editedAt);
    }
  }, [data, form?.key, urlKey]);
  const current = form?.key === urlKey ? form : undefined;
  const terms = useMemo(() => parseTerms(termsInput), [termsInput]);
  const prefix = prefixText(prefixKind, prefixDate, prefixCustom, lang);

  // Messwerte bei jedem Tastendruck vom Kern (< 16 ms); veraltete Antworten werden verworfen.
  useEffect(() => {
    if (!current) return;
    const controller = new AbortController();
    void detail.serpMeasure({ title: current.title, description: current.description, device, boldTerms: terms, prefix }, controller.signal)
      .then(value => { if (!controller.signal.aborted) setMeasure(value.rows); })
      .catch(error => { if (!controller.signal.aborted && !isSilentError(error)) actions.reportError(error); });
    return () => controller.abort();
  }, [current?.title, current?.description, device, terms, prefix, current?.key]);

  // Bearbeitungen werden je URL im Crawl gespeichert (Vorschau, ändert nie die Website); gleiche Werte wie gecrawlt verwerfen die Bearbeitung.
  const baseline = useRef<{ key: string; title: string; description: string; edited: boolean } | undefined>(undefined);
  useEffect(() => { if (data && current) baseline.current = baseline.current?.key === urlKey ? baseline.current : { key: urlKey, title: data.title, description: data.description, edited: !!data.edited }; }, [data, current, urlKey]);
  const dirty = useRef(false);
  useEffect(() => {
    if (!current || !dirty.current) return;
    const original = baseline.current;
    const timer = setTimeout(() => {
      dirty.current = false;
      if (original && current.title === original.title && current.description === original.description) {
        if (original.edited) { original.edited = false; setEditedAt(undefined); void detail.serpReset({ urlId: focus }).catch(actions.reportError); }
        return;
      }
      if (original) original.edited = true;
      setEditedAt(new Date().toISOString());
      void detail.serpSaveEdit({ urlId: focus, title: current.title, description: current.description }).catch(actions.reportError);
    }, SAVE_DELAY_MS);
    return () => clearTimeout(timer);
  }, [current?.title, current?.description]);
  const edit = (next: Partial<Pick<Form, "title" | "description">>) => { dirty.current = true; setForm(previous => previous ? { ...previous, ...next } : previous); };

  if (result.error) return <DetailError error={result.error} />;
  if (!data || !current) return <DetailLoading />;
  const [titleRow, descriptionRow] = [measure[0], measure[1]];
  const fullDescription = prefix + current.description;
  const shownTitle = displayText(current.title, titleRow);
  const shownDescription = displayText(fullDescription, descriptionRow);
  const prefixLength = Math.min(codepoints(prefix).length, codepoints(shownDescription).length);
  const shownPrefix = codepoints(shownDescription).slice(0, prefixLength).join("");
  const shownBody = codepoints(shownDescription).slice(prefixLength).join("");
  const edited = editedAt !== undefined;
  const num = (value: number) => new Intl.NumberFormat(lang, { maximumFractionDigits: 1 }).format(value);
  const reset = () => {
    dirty.current = false;
    setForm({ key: urlKey, title: data.title, description: data.description }); setEditedAt(undefined);
    if (baseline.current) baseline.current.edited = false;
    void detail.serpReset({ urlId: focus }).catch(actions.reportError);
  };
  const rows: [string, string, SerpMeasureRow | undefined][] = [["title", t("serp-title"), titleRow], ["description", t("serp-description"), descriptionRow]];
  return <div className="os-fach os-serp">
    <div className="os-fach-toolbar">
      <RadioGroup label={t("serp-device")} orientation="horizontal" value={device} onChange={value => setDevice(value as SerpDevice)} options={SERP_DEVICES.map(value => ({ value, label: t(`serp-device-${value}`) }))} />
      <TextField label={t("serp-bold-terms")} value={termsInput} onChange={setTermsInput} description={t("serp-bold-terms-hint")} />
      <RadioGroup label={t("serp-prefix")} orientation="horizontal" value={prefixKind} onChange={value => setPrefixKind(value as SerpPrefixKind)}
        options={(["none", "date", "custom"] as const).map(value => ({ value, label: t(`serp-prefix-${value}`) }))} />
      {prefixKind === "date" && <label className="os-serp-date"><span className="os-label">{t("serp-prefix-date-value")}</span><input className="os-input" type="date" value={prefixDate} onChange={event => setPrefixDate(event.target.value)} /></label>}
      {prefixKind === "custom" && <TextField label={t("serp-prefix-custom-value")} value={prefixCustom} onChange={setPrefixCustom} />}
    </div>
    <section className="os-serp-preview" data-device={device} aria-label={t("serp-preview")}>
      <div className="os-serp-site">
        {data.favicon ? <img className="os-serp-favicon" src={data.favicon} alt="" width={18} height={18} /> : <span className="os-serp-favicon os-serp-favicon-fallback" aria-hidden />}
        <span className="os-serp-host">{data.host}</span><span className="os-serp-breadcrumb">{data.breadcrumb}</span>
        {edited && <span className="os-serp-edited" title={t("serp-edited")}><Pencil size={12} aria-hidden /><span>{t("serp-edited")}</span></span>}
      </div>
      <p className="os-serp-title" data-truncated={titleRow?.truncated ? "true" : "false"}><Rich text={shownTitle} terms={terms} /></p>
      <p className="os-serp-description" data-truncated={descriptionRow?.truncated ? "true" : "false"}>
        {shownPrefix && <span className="os-serp-prefix">{shownPrefix}</span>}<Rich text={shownBody} terms={terms} />
      </p>
    </section>
    <div className="os-serp-fields">
      <TextField label={t("serp-edit-title")} value={current.title} onChange={title => edit({ title })} />
      <label className="os-field"><span className="os-label">{t("serp-edit-description")}</span>
        <textarea className="os-fach-textarea" rows={3} value={current.description} onChange={event => edit({ description: event.target.value })} aria-describedby={current.description.trim() === "" ? "serp-no-description" : undefined} /></label>
      {current.description.trim() === "" && <Banner kind="info"><span id="serp-no-description">{t("serp-no-description")}</span></Banner>}
      <div className="os-fach-actions"><Button onPress={reset} isDisabled={!edited && current.title === data.title && current.description === data.description}>{t("serp-reset")}</Button></div>
    </div>
    <table className="os-fach-table os-serp-metrics"><caption>{t("serp-metrics")}</caption>
      <thead><tr><th scope="col">{t("serp-field")}</th><th scope="col" className="os-fach-num">{t("serp-chars")}</th><th scope="col" className="os-fach-num">{t("serp-px")}</th>
        <th scope="col" className="os-fach-num">{t("serp-max-px")}</th><th scope="col" className="os-fach-num">{t("serp-remaining-px")}</th><th scope="col">{t("serp-truncated")}</th></tr></thead>
      <tbody>{rows.map(([id, label, row]) => <tr key={id} data-metric={id}>
        <th scope="row">{label}</th><td className="os-fach-num">{row ? row.chars : "–"}</td><td className="os-fach-num">{row ? num(row.px) : "–"}</td>
        <td className="os-fach-num">{row ? num(row.maxPx) : "–"}</td><td className="os-fach-num" data-negative={row && row.remaining < 0 ? "true" : undefined}>{row ? num(row.remaining) : "–"}</td>
        <td>{row ? (row.truncated ? t("serp-yes") : t("serp-no")) : "–"}</td></tr>)}</tbody></table>
    <p className="os-fach-note">{t("serp-estimate-hint")}</p>
  </div>;
}

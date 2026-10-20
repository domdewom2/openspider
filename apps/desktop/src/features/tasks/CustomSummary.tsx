import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { exportApi } from "../../ipc/export";
import type { CustomSummaryArgs } from "../../ipc/gen/CustomSummaryArgs";
import type { CustomSummaryMetric } from "../../ipc/gen/CustomSummaryMetric";
import { Button, Checkbox, TextField } from "../../ui/primitives";
import { taskError } from "./model";

export function CustomSummary({ value, onChange }: { value: CustomSummaryArgs | null; onChange(value: CustomSummaryArgs | null): void }) {
  const t = useT();
  const [catalog, setCatalog] = useState<CustomSummaryMetric[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [text, setText] = useState("");
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    const abort = new AbortController();
    setError(null);
    void exportApi.customSummaryMetrics(abort.signal).then(setCatalog).catch(e => { if (!abort.signal.aborted) setError(taskError(e)); });
    return () => abort.abort();
  }, [attempt]);
  const metrics = value?.metrics ?? [];
  const update = (next: string[]) => onChange({ account: "", target: "", ...value, metrics: next });
  const move = (index: number, by: number) => {
    const next = [...metrics];
    [next[index], next[index + by]] = [next[index + by], next[index]];
    update(next);
  };
  return <div className="os-tasks-fields">
    <Checkbox isSelected={value !== null} onChange={enabled => onChange(enabled ? { account: "", target: "", metrics: [] } : null)}>{t("tasks-summary-enable")}</Checkbox>
    {value && <>
      <TextField label={t("tasks-google-account")} value={value.account} onChange={account => onChange({ ...value, account })} />
      <TextField label={t("tasks-summary-target")} value={value.target} onChange={target => onChange({ ...value, target })} />
      <p>{t("tasks-summary-columns")}</p>
      <TextField label={t("tasks-metric-search")} value={search} onChange={setSearch} />
      {error && <p role="alert">{error} <Button onPress={() => setAttempt(n => n + 1)}>{t("tasks-retry")}</Button></p>}
      <fieldset><legend>{t("tasks-available")}</legend>
        {catalog?.filter(m => !metrics.includes(m.id) && m.id.toLocaleLowerCase().includes(search.toLocaleLowerCase())).map(m =>
          <Button key={m.id} onPress={() => update([...metrics, m.id])}>{m.id}</Button>)}
      </fieldset>
      <ol aria-label={t("tasks-selected")}>
        {metrics.map((name, index) => <li key={name}>{name}
          {catalog && !catalog.some(m => m.id === name) && <span role="status"> {t("tasks-metric-unavailable", { name })}</span>}
          <Button aria-label={t("tasks-move-up", { name })} isDisabled={index === 0} onPress={() => move(index, -1)}>↑</Button>
          <Button aria-label={t("tasks-move-down", { name })} isDisabled={index === metrics.length - 1} onPress={() => move(index, 1)}>↓</Button>
          <Button aria-label={t("tasks-remove-name", { name })} onPress={() => update(metrics.filter(m => m !== name))}>{t("tasks-remove")}</Button>
        </li>)}
      </ol>
      <label className="os-field"><span className="os-label">{t("tasks-metric-text")}</span><textarea className="os-input" value={text} onChange={e => setText(e.target.value)} /></label>
      <div className="os-tasks-actions">
        <Button onPress={() => { update([...new Set(text.split(/\r?\n/).map(m => m.trim()).filter(Boolean))]); }}>{t("tasks-paste-metrics")}</Button>
        <Button onPress={() => setText(metrics.join("\n"))}>{t("tasks-copy-metrics")}</Button>
      </div>
    </>}
  </div>;
}

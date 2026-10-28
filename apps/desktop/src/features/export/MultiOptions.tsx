import { useEffect, useState } from "react";
import { useT, i18n, currentLanguage } from "../../i18n";
import { filterLabel, tabLabel } from "../../i18n/canon";
import { catalog } from "../../ipc/catalog";
import { exportApi } from "../../ipc/export";
import type { ExportSel } from "../../ipc/gen/ExportSel";
import type { ExportPreset } from "../../ipc/gen/ExportPreset";
import { Button, Checkbox, Select, TextField } from "../../ui/primitives";
import type { ExportChoices } from "./execute";

interface Choice extends ExportSel { label: string; enabled: boolean; reason?: string | null }
interface Group { id: string; label: string; items: Choice[] }
const id = (item: ExportSel) => JSON.stringify([item.kind, item.name]);

/**
 * Selection of the multi export (OUT-075, A02-17): a searchable tree of all tabs/filters, bulk exports and reports on the left,
 * the selected exports in their order on the right. Loading failures of one source do not empty the other ones; each source
 * names its error with a retry (the empty tree in the beta was not explained).
 */
export function MultiOptions({ value, onChange, onReady, onError, showPresets = true, showSkipEmpty = true }: {
  value: ExportChoices; onChange(value: ExportChoices): void; onReady(ready: boolean): void; onError(error: unknown): void;
  /** The multi export dialog places presets and „skip empty“ in sections of their own; the task editor keeps them here. */
  showPresets?: boolean; showSkipEmpty?: boolean;
}) {
  const t = useT();
  const [groups, setGroups] = useState<{ tabs: Group[]; exports: Group[] }>({ tabs: [], exports: [] });
  const [failed, setFailed] = useState<{ tabs?: unknown; exports?: unknown }>({});
  const [loading, setLoading] = useState(true);
  const [attempt, setAttempt] = useState(0);
  const [search, setSearch] = useState("");
  useEffect(() => {
    let live = true;
    onReady(false); setLoading(true);
    const label = (key: string, fallback: string) => i18n.has(currentLanguage(), key) ? t(key) : fallback;
    void Promise.allSettled([catalog.tabs(), exportApi.catalog()]).then(([tabs, exports]) => {
      if (!live) return;
      setGroups({
        tabs: tabs.status === "fulfilled" ? tabs.value.map(tab => ({ id: `tab:${tab.id}`, label: tabLabel(t, tab.id), items: tab.filters.map(f => ({ kind: "tab", name: `${tab.id}:${f.id}`, label: filterLabel(t, f), enabled: f.state.kind === "ready", reason: f.state.kind === "ready" ? null : t("export-unavailable") })) })) : [],
        exports: exports.status === "fulfilled" ? (["bulk", "reports"] as const).flatMap(key => exports.value[key].map(group => ({ id: `${key}:${group.key}`, label: label(group.labelKey, group.key), items: group.items.map(item => ({ kind: key === "reports" ? "report" : "bulk", name: item.name, label: label(item.labelKey, item.name), enabled: item.enabled, reason: item.disabledReason ? t(item.disabledReason) : null })) }))) : [],
      });
      setFailed({ tabs: tabs.status === "rejected" ? tabs.reason : undefined, exports: exports.status === "rejected" ? exports.reason : undefined });
      setLoading(false);
      onReady(true);
      if (tabs.status === "rejected") onError(tabs.reason);
      else if (exports.status === "rejected") onError(exports.reason);
    });
    return () => { live = false; };
  }, [t, onReady, onError, attempt]);
  const all = [...groups.tabs, ...groups.exports];
  const selected = new Set(value.items.map(id));
  const known = new Map(all.flatMap(g => g.items.map(item => [id(item), { item, group: g }] as const)));
  const warnings = value.items.filter(item => !known.get(id(item))?.item.enabled);
  const set = (items: ExportSel[]) => onChange({ ...value, items });
  const toggle = (item: ExportSel, checked: boolean) => set(checked ? [...value.items, { kind: item.kind, name: item.name }] : value.items.filter(v => id(v) !== id(item)));
  const needle = search.trim().toLocaleLowerCase();
  const visible = (group: Group) => group.items.filter(item => !needle || `${group.label} ${item.label} ${item.name}`.toLocaleLowerCase().includes(needle));
  const nameOf = (item: ExportSel) => { const entry = known.get(id(item)); return entry ? `${entry.group.label} › ${entry.item.label}` : item.name; };
  return <div className="os-export-fields">
    <div className="os-export-picker">
      <div className="os-export-pane">
        <TextField label={t("export-search")} value={search} onChange={setSearch} />
        <div className="os-export-tree" role="group" aria-label={t("export-available")} aria-busy={loading}>
          {loading && <p role="status">{t("export-loading")}</p>}
          {(["tabs", "exports"] as const).map(source => failed[source] ? <p key={source} role="alert">{t(source === "tabs" ? "export-tabs-failed" : "export-catalog-failed")} <Button onPress={() => setAttempt(n => n + 1)}>{t("export-retry")}</Button></p> : null)}
          {all.map(group => {
            const items = visible(group);
            if (!items.length) return null;
            const enabled = items.filter(item => item.enabled);
            const count = group.items.filter(item => selected.has(id(item))).length;
            return <details key={group.id} open><summary><span>{group.label}</span>{count > 0 && <span className="os-export-count"> ({t("export-group-count", { count, total: group.items.length })})</span>}</summary>
              <fieldset><legend className="os-export-sr">{group.label}</legend>
                {enabled.length > 1 && <Button className="os-export-group-all" aria-label={t("export-group-all", { group: group.label })}
                  onPress={() => set([...value.items, ...enabled.filter(item => !selected.has(id(item))).map(({ kind, name }) => ({ kind, name }))])}>{t("export-group-all-short")}</Button>}
                {items.map(item => <div key={id(item)} title={item.reason ?? undefined}>
                  <Checkbox isSelected={selected.has(id(item))} isDisabled={!item.enabled && !selected.has(id(item))} onChange={checked => toggle(item, checked)}>{item.label}</Checkbox>
                  {!item.enabled && <small>{item.reason ?? t("export-unavailable")}</small>}
                </div>)}
              </fieldset></details>;
          })}
          {!loading && needle && !all.some(group => visible(group).length) && <p role="status">{t("export-no-match")}</p>}
        </div>
      </div>
      <div className="os-export-pane">
        <div className="os-export-pane-head"><span className="os-export-pane-title" id="os-export-selected-title">{t("export-selected", { count: value.items.length })}</span>
          <Button isDisabled={!value.items.length} onPress={() => set([])}>{t("export-remove-all")}</Button></div>
        <ol className="os-export-selected" aria-labelledby="os-export-selected-title">
          {value.items.map(item => <li key={id(item)}><span>{nameOf(item)}</span>
            <Button aria-label={t("export-remove", { name: nameOf(item) })} onPress={() => toggle(item, false)}>×</Button></li>)}
        </ol>
        {!value.items.length && <p className="os-export-hint">{t("export-selected-empty")}</p>}
      </div>
    </div>
    {warnings.length > 0 && <p role="status">{t("export-skipped-warning")} {warnings.map(item => item.name).join(", ")}</p>}
    {showPresets && <PresetControls value={value} onChange={onChange} onError={onError} />}
    {showSkipEmpty && <Checkbox isSelected={value.skipEmpty} onChange={skipEmpty => onChange({ ...value, skipEmpty })}>{t("export-skip-empty")}</Checkbox>}
  </div>;
}

/** Named presets (`.ospreset`, OUT-075): apply, save under a name, delete. */
export function PresetControls({ value, onChange, onError }: { value: ExportChoices; onChange(value: ExportChoices): void; onError(error: unknown): void }) {
  const t = useT();
  const [presets, setPresets] = useState<ExportPreset[]>([]);
  const [preset, setPreset] = useState("");
  const [presetName, setPresetName] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let live = true;
    void exportApi.presetList().then(saved => { if (live) setPresets(saved); }).catch(error => { if (live) onError(error); });
    return () => { live = false; };
  }, [onError]);
  const act = async (operation: () => Promise<void>) => {
    setBusy(true);
    try { await operation(); } catch (error) { onError(error); } finally { setBusy(false); }
  };
  return <div className="os-export-fields">
    <Select label={t("export-preset")} value={preset} options={[{ value: "", label: t("export-no-preset") }, ...presets.map(p => ({ value: p.name, label: p.name }))]} onChange={name => {
      setPreset(name);
      const found = presets.find(p => p.name === name);
      if (found) { setPresetName(name); onChange({ ...value, items: found.items, singleWorkbook: found.singleWorkbook, skipEmpty: found.skipEmpty }); }
    }} />
    <TextField label={t("export-preset-name")} value={presetName} onChange={setPresetName} />
    <div className="os-export-actions">
      <Button isDisabled={busy || !presetName.trim() || !value.items.length} onPress={() => void act(async () => {
        const name = presetName.trim();
        const saved: ExportPreset = { name, items: value.items, singleWorkbook: value.singleWorkbook, skipEmpty: value.skipEmpty };
        await exportApi.presetSave({ name, preset: saved });
        setPresets(await exportApi.presetList()); setPreset(name);
      })}>{t("export-preset-save")}</Button>
      <Button isDisabled={busy || !preset} onPress={() => void act(async () => {
        await exportApi.presetDelete({ name: preset });
        setPresets(await exportApi.presetList()); setPreset("");
      })}>{t("export-preset-delete")}</Button>
    </div>
  </div>;
}

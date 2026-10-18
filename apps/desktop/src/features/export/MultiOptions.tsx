import { useEffect, useState } from "react";
import { useT, i18n, currentLanguage } from "../../i18n";
import { catalog } from "../../ipc/catalog";
import { exportApi } from "../../ipc/export";
import type { ExportSel } from "../../ipc/gen/ExportSel";
import type { ExportPreset } from "../../ipc/gen/ExportPreset";
import { Button, Checkbox, Select, TextField } from "../../ui/primitives";
import type { ExportChoices } from "./execute";

interface Choice extends ExportSel { label: string; enabled: boolean; reason?: string | null }
interface Group { id: string; label: string; items: Choice[] }
const id = (item: ExportSel) => JSON.stringify([item.kind, item.name]);

export function MultiOptions({ value, onChange, onReady, onError }: {
  value: ExportChoices; onChange(value: ExportChoices): void; onReady(ready: boolean): void; onError(error: unknown): void;
}) {
  const t = useT();
  const [groups, setGroups] = useState<Group[]>([]);
  const [presets, setPresets] = useState<ExportPreset[]>([]);
  const [preset, setPreset] = useState("");
  const [presetName, setPresetName] = useState("");
  const [search, setSearch] = useState("");
  const [busy, setBusy] = useState(false);
  useEffect(() => {
    let live = true;
    onReady(false);
    const label = (key: string, fallback: string) => i18n.has(currentLanguage(), key) ? t(key) : fallback;
    void Promise.all([catalog.tabs(), exportApi.catalog(), exportApi.presetList()]).then(([tabs, exports, saved]) => {
      if (!live) return;
      setGroups([
        ...tabs.map(tab => ({ id: `tab:${tab.id}`, label: tab.id, items: tab.filters.map(f => ({ kind: "tab", name: `${tab.id}:${f.id}`, label: f.id, enabled: f.state.kind === "ready", reason: f.state.kind === "ready" ? null : t("export-unavailable") })) })),
        ...(["bulk", "reports"] as const).flatMap(key => exports[key].map(group => ({ id: `${key}:${group.key}`, label: label(group.labelKey, group.key), items: group.items.map(item => ({ kind: key === "reports" ? "report" : "bulk", name: item.name, label: label(item.labelKey, item.name), enabled: item.enabled, reason: item.disabledReason ? t(item.disabledReason) : null })) }))),
      ]);
      setPresets(saved);
      onReady(true);
    }).catch(error => { if (live) onError(error); });
    return () => { live = false; };
  }, [t, onReady, onError]);
  const selected = new Set(value.items.map(id));
  const known = new Map(groups.flatMap(g => g.items).map(item => [id(item), item]));
  const warnings = value.items.filter(item => !known.get(id(item))?.enabled);
  const act = async (operation: () => Promise<void>) => {
    setBusy(true);
    try { await operation(); } catch (error) { onError(error); } finally { setBusy(false); }
  };
  return <div className="os-export-fields">
    <TextField label={t("export-search")} value={search} onChange={setSearch} />
    <div className="os-export-tree" aria-label={t("export-selection")}>
      {groups.map(group => {
        const items = group.items.filter(item => `${group.label} ${item.label} ${item.name}`.toLocaleLowerCase().includes(search.toLocaleLowerCase()));
        return items.length ? <details key={group.id} open><summary>{group.label}</summary><fieldset><legend className="os-export-sr">{group.label}</legend>
          {items.map(item => <div key={id(item)} title={item.reason ?? undefined}>
            <Checkbox isSelected={selected.has(id(item))} isDisabled={!item.enabled && !selected.has(id(item))} onChange={checked => onChange({ ...value, items: checked ? [...value.items, { kind: item.kind, name: item.name }] : value.items.filter(v => id(v) !== id(item)) })}>{item.label}</Checkbox>
            {!item.enabled && <small>{item.reason ?? t("export-unavailable")}</small>}
          </div>)}
        </fieldset></details> : null;
      })}
    </div>
    {warnings.length > 0 && <p role="status">{t("export-skipped-warning")} {warnings.map(item => item.name).join(", ")}</p>}
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
    <Checkbox isSelected={value.skipEmpty} onChange={skipEmpty => onChange({ ...value, skipEmpty })}>{t("export-skip-empty")}</Checkbox>
  </div>;
}

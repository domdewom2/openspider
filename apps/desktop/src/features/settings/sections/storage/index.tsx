import { useEffect, useState } from "react";
import { useT } from "../../../../i18n";
import { app } from "../../../../ipc/app";
import { crawls } from "../../../../ipc/crawls";
import type { AppPaths } from "../../../../ipc/gen/AppPaths";
import { usePrefs } from "../../../../state/prefs";
import { useRelocation } from "./relocation";
import { Button, Dialog, DialogFooter, Select } from "../../../../ui/primitives";
import { RestartNote } from "../../frame/RestartNote";
import { sectionTitleKey, type SettingsSection } from "../../frame/types";
import { GiB, ramMaximum, readResources, type Resources } from "./resources";
import { SettingInput, SettingsError, useSettingsAction } from "./shared";

export function Storage() {
  const t = useT();
  const storage = usePrefs((s) => s.dto?.storage);
  const [resources, setResources] = useState<Resources | null>(null);
  const [paths, setPaths] = useState<AppPaths | null>(null);
  const [resourceError, setResourceError] = useState(false);
  const [candidate, setCandidate] = useState<string | null>(null);
  const [warnings, setWarnings] = useState<string[]>([]);
  const [moveTo, setMoveTo] = useState<string | null>(null);
  const relocation = useRelocation();
  const { status: move, opId: moveId } = relocation;
  const action = useSettingsAction();
  useEffect(() => {
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      try { const r = await readResources(); if (active) { setResources(r); setResourceError(false); } }
      catch { if (active) setResourceError(true); }
      if (active) timer = setTimeout(() => void poll(), 1000);
    }
    void poll();
    void app.info().then((i) => { if (active) setPaths(i.paths); }).catch(() => undefined);
    return () => { active = false; clearTimeout(timer); };
  }, []);
  async function acceptLocation(path: string) {
    await action.save({ storage: { crawlsDir: path } });
    setCandidate(null);
    setPaths((p) => p ? { ...p, crawls: path } : p);
    setMoveTo(path);
  }
  async function chooseLocation(path: string) {
    const notices = await crawls.checkLocation({ path });
    if (notices.length) { setWarnings(notices); setCandidate(path); }
    else await acceptLocation(path);
  }
  if (!storage) return null;
  const max = resources ? ramMaximum(resources.physicalBytes) : null;
  const inMove = relocation.busy;
  return <div className="os-settings-group">
    <p>{t("u14-storage-disk")}</p>
    <SettingInput label={t("u14-storage-directory")} path="storage.crawlsDir" chooser="directory" value={storage.crawlsDir ?? paths?.crawls ?? ""}
      disabled={action.busy || inMove} validate={(v) => v.trim() ? null : t("u14-path-required")} commit={chooseLocation} />
    <Select label={t("u14-storage-profile")} value={storage.performanceProfile} anchor="storage.performanceProfile" isDisabled={action.busy}
      options={["balanced", "max_speed", "min_memory"].map((value) => ({ value, label: t(`u14-profile-${value}`) }))}
      onChange={(performanceProfile) => void action.run(() => action.save({ storage: { performanceProfile } }))} />
    <RestartNote settingPath="storage.performance_profile" />
    <p className="os-settings-note" role="status">{resources ? t("u14-storage-usage", { used: resources.usedBytes / GiB, limit: resources.ramLimitBytes / GiB }) : t("u14-storage-unavailable")}</p>
    {resourceError && <p role="alert">{t("u14-storage-unavailable")}</p>}
    <SettingInput label={t("u14-storage-ram")} path="storage.ramLimitGb" numeric
      value={storage.ramLimitGb?.toFixed(1) ?? (resources ? (resources.ramLimitBytes / GiB).toFixed(1) : "")}
      disabled={!resources || action.busy} description={max === null ? t("u14-storage-unavailable") : t("u14-storage-range", { max })}
      validate={(v) => max !== null && v.trim() && Number.isFinite(Number(v)) && Number(v) >= 1 && Number(v) <= max && Math.abs(Number(v) * 10 - Math.round(Number(v) * 10)) < 1e-8 ? null : t("u14-storage-ram-invalid", { max: max ?? 1 })}
      commit={(v) => action.save({ storage: { ramLimitGb: Number(v) } })} />
    <h3>{t("u14-storage-resources")}</h3>
    <SettingInput label={t("u14-storage-cache")} path="storage.cacheLimitMb" numeric disabled={!resources || action.busy}
      value={String(storage.cacheLimitMb ?? resources?.cacheLimitMb ?? "")}
      validate={(v) => Number.isInteger(Number(v)) && Number(v) >= 512 && Number(v) <= 4294967295 ? null : t("u14-cache-invalid")}
      commit={(v) => action.save({ storage: { cacheLimitMb: Number(v) } })} />
    <SettingInput label={t("u14-storage-renderers")} path="storage.renderProcessLimit" numeric disabled={!resources || action.busy}
      value={String(storage.renderProcessLimit ?? resources?.renderProcessLimit ?? "")}
      validate={(v) => Number.isInteger(Number(v)) && Number(v) >= 1 && Number(v) <= 32 ? null : t("u14-renderers-invalid")}
      commit={(v) => action.save({ storage: { renderProcessLimit: Number(v) } })} />
    <SettingsError error={action.error ?? relocation.error} />
    <Dialog title={t("u14-location-warning")} isOpen={candidate !== null} onOpenChange={(open) => { if (!open) setCandidate(null); }}
      footer={<DialogFooter okLabel={t("u14-location-anyway")} cancelLabel={t("u14-location-other")} okDisabled={action.busy}
        onCancel={() => setCandidate(null)} onOk={() => { if (candidate) void action.run(() => acceptLocation(candidate)); }} />}>
      <p>{t("u14-location-risk")}</p><p>{candidate}</p><ul>{warnings.map((w) => <li key={w}>{t(({ "warn-location-sync": "u14-location-sync", "warn-location-network": "u14-location-network", "warn-location-filesystem": "u14-location-filesystem" } as Record<string, string>)[w] ?? "u14-location-risk")}</li>)}</ul>
      <SettingsError error={action.error} />
    </Dialog>
    <Dialog title={t("u14-move-title")} isOpen={moveTo !== null} onOpenChange={(open) => { if (!open) setMoveTo(null); }}
      footer={<DialogFooter okLabel={t("u14-move-now")} cancelLabel={t("u14-move-later")} okDisabled={action.busy}
        onCancel={() => setMoveTo(null)} onOk={() => void action.run(async () => {
          const rows = await crawls.list({ show: "crawls" });
          const ids = rows.filter((r) => r.crawl.phase !== "running").map((r) => r.crawl.crawlId);
          if (moveTo) await relocation.begin(ids, moveTo);
          setMoveTo(null);
        })} />}>
      <p>{t("u14-move-description")}</p><p>{moveTo}</p><SettingsError error={action.error} />
    </Dialog>
    {moveId && <div role="status">
      <p>{t(`u14-move-${inMove ? "running" : move?.state ?? "running"}`)}</p>
      {move && <progress aria-label={t("u14-move-progress")} value={move.done} {...(move.total ? { max: move.total } : {})} />}
      {inMove && <Button onPress={() => void action.run(() => relocation.cancel())}>{t("u14-cancel")}</Button>}
      {move?.error && <SettingsError error={move.error.detail ?? t("u14-save-error")} />}
    </div>}
  </div>;
}
export const section: SettingsSection = { id: "storage", order: 30, titleKey: sectionTitleKey("storage"), Component: Storage };

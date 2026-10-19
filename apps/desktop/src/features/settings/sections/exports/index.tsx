import { useEffect, useState } from "react";
import { useT } from "../../../../i18n";
import { app } from "../../../../ipc/app";
import { usePrefs } from "../../../../state/prefs";
import { Select } from "../../../../ui/primitives";
import { RestartNote } from "../../frame/RestartNote";
import { sectionTitleKey, type SettingsSection } from "../../frame/types";
import { SettingInput, SettingsError, useSettingsAction } from "../storage/shared";

export function Exports() {
  const t = useT();
  const settings = usePrefs((s) => s.dto?.export);
  const [directory, setDirectory] = useState<string | null>(null);
  const action = useSettingsAction();
  useEffect(() => { let active = true; void app.info().then((i) => { if (active) setDirectory(i.paths.exports); }).catch(() => undefined); return () => { active = false; }; }, []);
  if (!settings) return null;
  return <div className="os-settings-group">
    <SettingInput label={t("u14-export-directory")} path="exports.directory" chooser="directory" value={settings.directory ?? ""}
      description={t("u14-export-default", { path: directory ?? t("u14-export-os-directory") })}
      commit={async (v) => {
        await action.save({ export: { directory: v.trim() ? v : null } });
        try { setDirectory((await app.info()).paths.exports); } catch { setDirectory(v.trim() || null); }
      }} />
    <p className="os-settings-note">{t("u14-export-live")}</p>
    <Select label={t("u14-export-format")} value={settings.format} anchor="exports.format" isDisabled={action.busy}
      options={[{ value: "csv", label: "CSV" }, { value: "xlsx", label: "XLSX" }]}
      onChange={(format) => void action.run(() => action.save({ export: { format } }))} />
    <RestartNote settingPath="export.directory" /><RestartNote settingPath="export.format" />
    <SettingsError error={action.error} />
  </div>;
}
export const section: SettingsSection = { id: "exports", order: 50, titleKey: sectionTitleKey("exports"), Component: Exports };

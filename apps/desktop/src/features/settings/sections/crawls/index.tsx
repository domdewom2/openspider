import { useState } from "react";
import { useT } from "../../../../i18n";
import { usePrefs } from "../../../../state/prefs";
import { Dialog, DialogFooter, Select, TextField } from "../../../../ui/primitives";
import { RestartNote } from "../../frame/RestartNote";
import { sectionTitleKey, type SettingsSection } from "../../frame/types";
import { SettingInput, SettingsError, useSettingsAction } from "../storage/shared";

export function Crawls() {
  const t = useT();
  const retention = usePrefs((s) => s.dto?.retention);
  const [pending, setPending] = useState<string | null>(null);
  const [customDraft, setCustomDraft] = useState("30");
  const action = useSettingsAction();
  if (!retention) return null;
  function change(policy: string) {
    if (policy === "custom") { setCustomDraft(String(retention?.customDays || 30)); setPending(policy); }
    else if (retention?.policy === "never" && policy !== "never") setPending(policy);
    else void action.run(() => action.save({ retention: { policy } }));
  }
  const customValid = Number.isInteger(Number(customDraft)) && Number(customDraft) >= 1 && Number(customDraft) <= 4294967295;
  return <div className="os-settings-group">
    <Select label={t("u14-retention")} value={retention.policy} anchor="crawls.retention" isDisabled={action.busy}
      options={["never", "one_week", "one_month", "three_months", "six_months", "one_year", "custom"].map((value) => ({ value, label: t(`u14-retention-${value}`) }))} onChange={change} />
    {retention.policy === "custom" && <SettingInput label={t("u14-retention-days")} path="crawls.customDays" numeric value={String(retention.customDays)}
      validate={(v) => Number.isInteger(Number(v)) && Number(v) >= 1 && Number(v) <= 4294967295 ? null : t("u14-positive-days")}
      commit={(v) => action.save({ retention: { customDays: Number(v) } })} />}
    <p className="os-settings-note">{t("u14-retention-protection")}</p>
    <RestartNote settingPath="retention.policy" />
    <SettingsError error={action.error} />
    <Dialog title={t(pending === "custom" ? "u14-retention-custom-title" : "u14-retention-enable")} isOpen={pending !== null} onOpenChange={(open) => { if (!open) setPending(null); }}
      footer={<DialogFooter okLabel={t(pending === "custom" ? "u14-retention-custom-apply" : "u14-retention-enable")} cancelLabel={t("u14-cancel")} okDisabled={action.busy || (pending === "custom" && !customValid)}
        onCancel={() => setPending(null)} onOk={() => void action.run(async () => {
          await action.save({ retention: { policy: pending, ...(pending === "custom" ? { customDays: Number(customDraft) } : {}) } });
          setPending(null);
        })} />}>
      <p>{t("u14-retention-warning")}</p>
      {pending === "custom" && <TextField label={t("u14-retention-days")} type="number" value={customDraft} onChange={setCustomDraft}
        isDisabled={action.busy} error={customValid ? null : t("u14-positive-days")} />}
      <SettingsError error={action.error} />
    </Dialog>
  </div>;
}
export const section: SettingsSection = { id: "crawls", order: 40, titleKey: sectionTitleKey("crawls"), Component: Crawls };

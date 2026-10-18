import { RemovalConfirmationSettings } from "../../../table/RemovalConfirmationSettings";
import { useMemo } from "react";
import { i18n, useT } from "../../../../i18n";
import { usePrefs } from "../../../../state/prefs";
import { Select, Switch, type RadioOption } from "../../../../ui/primitives";
import { RestartNote } from "../../frame/RestartNote";
import type { SettingsSection } from "../../frame/types";
import { sectionTitleKey } from "../../frame/types";

/** Anzeigename einer Sprache in dieser Sprache selbst (Deutsch, English, …). */
export function languageName(lang: string): string {
  try {
    const n = new Intl.DisplayNames([lang], { type: "language" }).of(lang);
    return n ? n.charAt(0).toUpperCase() + n.slice(1) : lang;
  } catch {
    return lang;
  }
}

function General() {
  const t = useT();
  const languageSetting = usePrefs((s) => s.languageSetting);
  const notifications = usePrefs((s) => s.notifications);
  const patch = usePrefs((s) => s.patch);
  const options = useMemo<RadioOption[]>(
    () => [{ value: "system", label: t("settings-general-language-system") }, ...i18n.languages().map((l) => ({ value: l, label: languageName(l) }))],
    [t],
  );
  return (
    <div className="os-settings-group">
      <Select
        label={t("settings-general-language")}
        value={languageSetting}
        options={options}
        anchor="general.language"
        onChange={(v) => void patch({ general: { language: v } })}
      />
      <p className="os-settings-note">{t("settings-general-language-note")}</p>
      <RestartNote settingPath="general.language" />
      <Switch isSelected={notifications} anchor="general.notifications" onChange={(v) => void patch({ general: { notifications: v } })}>
        {t("settings-general-notifications")}
      </Switch>
      <RemovalConfirmationSettings />
    </div>
  );
}

export const section: SettingsSection = { id: "general", order: 10, titleKey: sectionTitleKey("general"), Component: General };

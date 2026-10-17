import { useT } from "../../../i18n";
import { usePrefs } from "../../../state/prefs";

/** Hinweis „Neustart erforderlich“ an Einstellungen, die der Kern als neustartpflichtig meldet (OUT-222). */
export function RestartNote({ settingPath }: { settingPath: string }) {
  const t = useT();
  const required = usePrefs((s) => s.restartRequired);
  if (!required.includes(settingPath)) return null;
  return (
    <p className="os-settings-note" role="status" data-restart-required={settingPath}>
      {t("settings-restart-required")}
    </p>
  );
}

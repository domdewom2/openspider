// OUT-247: Einstellung „Nach einem Absturz fragen …“ (`crash_reports.prompt`, Standard an) auf der Seite Allgemein.
// Richtlinie DisableCrashReports gesetzt → Schalter ausgegraut mit „Durch Administrator deaktiviert“.
import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { crash } from "../../ipc/crash";
import { usePrefs } from "../../state/prefs";
import { Switch } from "../../ui/primitives";

export function CrashPromptSetting() {
  const t = useT();
  const prompt = usePrefs((s) => s.dto?.crashReports?.prompt !== false);
  const patch = usePrefs((s) => s.patch);
  const [disabledByPolicy, setDisabledByPolicy] = useState(false);
  useEffect(() => {
    const abort = new AbortController();
    crash.overview(abort.signal).then((o) => { if (!abort.signal.aborted) setDisabledByPolicy(o.disabledByPolicy); }, () => undefined);
    return () => abort.abort();
  }, []);
  return <>
    <Switch isSelected={prompt && !disabledByPolicy} isDisabled={disabledByPolicy} anchor="crash_reports.prompt"
      onChange={(v) => void patch({ crashReports: { prompt: v } })}>
      {t("crash-settings-prompt")}
    </Switch>
    <p className="os-settings-note">{t(disabledByPolicy ? "crash-disabled-admin" : "crash-settings-note")}</p>
  </>;
}

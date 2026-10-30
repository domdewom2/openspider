import { useEffect, useState } from "react";
import { useT } from "../../../../i18n";
import { update } from "../../../../ipc/update";
import { usePrefs } from "../../../../state/prefs";
import { Switch } from "../../../../ui/primitives";
import type { SettingsSection } from "../../frame/types";
function Updates() {
  const t = useT();
  const check = usePrefs(s => s.dto?.updates.check ?? true);
  const [environment, setEnvironment] = useState(false);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let live = true;
    void update.status().then(state => { if (live) setEnvironment(typeof state === "object" && "disabled" in state && state.disabled === "environment"); }).catch(() => undefined);
    return () => { live = false; };
  }, []);
  const setAuto = async (check: boolean) => {
    try { await update.set_auto({ check }); await usePrefs.getState().load(); setFailed(false); }
    catch { setFailed(true); }
  };
  return <div className="os-settings-group">
    <Switch isSelected={check} isDisabled={environment} anchor="updates.check" onChange={check => void setAuto(check)}>{t("update-auto")}</Switch>
    {environment && <p>{t("update-environment")}</p>}
    <p>{t("update-privacy")}</p>
    {failed && <p role="alert">{t("update-failed")}</p>}

  </div>;
}
export const section: SettingsSection = { id: "updates", order: 95, titleKey: "update-title", Component: Updates };

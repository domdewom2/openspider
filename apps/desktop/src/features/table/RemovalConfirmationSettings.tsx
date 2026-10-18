import { useEffect, useState } from "react";
import { useT } from "../../i18n";
import { uistate } from "../../ipc/uistate";
import { object, saveTableState } from "../../state/tabs";
export async function confirmsRemoval(): Promise<boolean> {
  return object((await uistate.load()).state).tableConfirmRemove !== false;
}
/** Additive settings field; keyboard removal always confirms regardless of this preference. */
export function RemovalConfirmationSettings() {
  const t = useT(); const [enabled, setEnabled] = useState(true); const [error, setError] = useState(false);
  useEffect(() => {
    let live = true; void confirmsRemoval().then(value => { if (live) setEnabled(value); }).catch(() => { if (live) setError(true); });
    return () => { live = false; };
  }, []);
  return <div data-setting-anchor="general.tableConfirmRemove">
    <label><input type="checkbox" checked={enabled} onChange={event => {
      const value = event.target.checked; setEnabled(value);
      void saveTableState({ tableConfirmRemove: value }).catch(() => { setError(true); setEnabled(!value); });
    }} />{t("u10-confirm-removal-setting")}</label>
    {error && <p role="alert">{t("u10-confirm-removal-error")}</p>}
  </div>;
}

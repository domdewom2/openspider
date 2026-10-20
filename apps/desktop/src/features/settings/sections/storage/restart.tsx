import { create } from "zustand";
import { useT } from "../../../../i18n";
import { usePrefs } from "../../../../state/prefs";

/** Cache immediate settings.set requirements; the resource snapshot restores the app-wide
 * pending set when the settings window is reopened. A new core starts with no pending keys.
 */
export const usePendingRestarts = create<{ keys: string[] }>(() => ({ keys: [] }));
export function rememberPendingRestarts(restartRequired: readonly string[]) {
  const { keys } = usePendingRestarts.getState();
  const added = restartRequired.map((key) => key.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`)).filter((key) => !keys.includes(key));
  if (added.length) usePendingRestarts.setState({ keys: [...keys, ...added] });
}
usePrefs.subscribe(({ restartRequired }) => rememberPendingRestarts(restartRequired));

export function RestartNote({ settingPath }: { settingPath: string }) {
  const t = useT();
  const required = usePendingRestarts((s) => s.keys.includes(settingPath));
  if (!required) return null;
  return <p className="os-settings-note" role="status" data-restart-required={settingPath}>
    {t("settings-restart-required")}
  </p>;
}

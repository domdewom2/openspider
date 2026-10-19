import { useState } from "react";
import { useT } from "../../../../i18n";
import { client } from "../../../../ipc/client";
import { toCmdError } from "../../../../ipc/errors";
import { usePrefs } from "../../../../state/prefs";
import { Button, TextField } from "../../../../ui/primitives";
import { RestartNote } from "../../frame/RestartNote";

export function useSettingsAction() {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const t = useT();
  async function run(action: () => Promise<void>) {
    setBusy(true);
    setError(null);
    try { await action(); }
    catch (e) { const err = toCmdError(e); setError(err.detail ?? t("u14-save-error")); }
    finally { setBusy(false); }
  }
  async function save(patch: Record<string, unknown>) {
    await usePrefs.getState().patch(patch);
    const error = usePrefs.getState().error;
    if (error) throw error;
  }
  return { busy, error, run, save };
}

export function SettingsError({ error }: { error: string | null }) {
  return error ? <p className="os-error" role="alert">{error}</p> : null;
}

/** Local draft keeps incomplete input out of settings.toml. Each field commits independently. */
export function SettingInput({ label, value, path, commit, validate, description, numeric = false, disabled = false, chooser }: {
  label: string; value: string; path: string; commit: (value: string) => Promise<void>;
  validate?: (value: string) => string | null; description?: string; numeric?: boolean; disabled?: boolean; chooser?: "directory" | "certificate";
}) {
  const t = useT();
  const [draft, setDraft] = useState(value);
  const [seen, setSeen] = useState(value);
  if (seen !== value) { setSeen(value); if (draft === seen) setDraft(value); }
  const action = useSettingsAction();
  const invalid = draft === value ? null : validate?.(draft) ?? null;
  return <div className="os-settings-group">
    <TextField label={label} value={draft} onChange={setDraft} anchor={path}
      {...(description ? { description } : {})} error={invalid} isDisabled={disabled || action.busy}
      {...(numeric ? { type: "number", inputMode: "decimal" } as const : {})} />
    {chooser && client.transportName === "tauri" && <Button isDisabled={disabled || action.busy} onPress={() => void action.run(async () => {
      const selected = await client.invoke<string | null>("dialog.open", { options: {
        title: label, directory: chooser === "directory", multiple: false,
        ...(draft ? { defaultPath: draft } : {}),
        ...(chooser === "certificate" ? { filters: [{ name: t("u14-certificates"), extensions: ["crt", "pem", "cer", "der"] }] } : {}),
      } });
      if (selected) { setDraft(selected); await commit(selected); }
    })}>{t("u14-browse")}</Button>}
    <Button isDisabled={disabled || action.busy || !!invalid || draft === value}
      onPress={() => void action.run(() => commit(draft))}>{t("u14-apply", { field: label })}</Button>
    <SettingsError error={action.error} />
    <RestartNote settingPath={path.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`)} />
  </div>;
}

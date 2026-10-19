import { useEffect, useState } from "react";
import { useT } from "../../../../i18n";
import { api } from "../../../../ipc/api";
import type { ApiSourceInfo } from "../../../../ipc/gen/ApiSourceInfo";
import { usePrefs } from "../../../../state/prefs";
import { Button, Select, Switch } from "../../../../ui/primitives";
import { RestartNote } from "../storage/restart";
import { sectionTitleKey, type SettingsSection } from "../../frame/types";
import { SettingsError, useSettingsAction } from "../storage/shared";

export function Accounts() {
  const t = useT();
  const accounts = usePrefs((s) => s.dto?.accounts);
  const [sources, setSources] = useState<ApiSourceInfo[]>([]);
  const action = useSettingsAction();
  useEffect(() => { let active = true; void action.run(async () => { const list = await api.list(); if (active) setSources(list); }); return () => { active = false; }; }, []);
  if (!accounts) return null;
  return <div className="os-settings-group">
    <p>{t("u14-accounts-note")}</p>
    <Select label={t("u14-accounts-notify")} value={accounts.notifyAccount ?? ""} anchor="accounts.notifyAccount" isDisabled={action.busy}
      options={[{ value: "", label: t("u14-accounts-none") }, ...accounts.mailAccounts.map((id) => ({ value: id, label: id }))]}
      onChange={(notifyAccount) => void action.run(() => action.save({ accounts: { notifyAccount: notifyAccount || null } }))} />
    {!sources.length && !action.busy && <p>{t("u14-accounts-empty")}</p>}
    {sources.map((source) => <div key={source.kind} className="os-settings-group">
      <h3>{source.kind.toUpperCase()}</h3>
      <p>{t(source.connected ? "u14-accounts-connected" : "u14-accounts-disconnected")}{source.account ? ` · ${source.account}` : ""}</p>
      <Switch isSelected={accounts.autoConnect.includes(source.kind)} anchor={`accounts.${source.kind}`} isDisabled={action.busy}
        onChange={(enabled) => void action.run(() => action.save({ accounts: { autoConnect: enabled ? [...new Set([...accounts.autoConnect, source.kind])] : accounts.autoConnect.filter((k) => k !== source.kind) } }))}>
        {t("u14-accounts-auto", { source: source.kind.toUpperCase() })}
      </Switch>
      {source.accounts.map((account) => <p key={account.accountId}>{account.account}{account.keySuffix ? ` · …${account.keySuffix}` : ""}</p>)}
      {source.connected && <Button isDisabled={action.busy} onPress={() => void action.run(async () => {
        await api.disconnect({ kind: source.kind }); setSources(await api.list());
      })}>{t("u14-accounts-disconnect", { source: source.kind.toUpperCase() })}</Button>}
    </div>)}
    <RestartNote settingPath="accounts.auto_connect" /><RestartNote settingPath="accounts.notify_account" />
    <SettingsError error={action.error} />
  </div>;
}
export const section: SettingsSection = { id: "accounts", order: 70, titleKey: sectionTitleKey("accounts"), Component: Accounts };

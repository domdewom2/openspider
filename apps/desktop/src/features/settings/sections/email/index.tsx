import { useCallback, useEffect, useRef, useState } from "react";
import { useT } from "../../../../i18n";
import { mail } from "../../../../ipc/mail";
import type { MailAccountInput } from "../../../../ipc/gen/MailAccountInput";
import type { MailAccountInfo } from "../../../../ipc/gen/MailAccountInfo";
import type { MailTestResult } from "../../../../ipc/gen/MailTestResult";
import type { OpHandle } from "../../../../ipc/gen/OpHandle";
import { useSession } from "../../../../state/session";
import { Button, Checkbox, Dialog, DialogFooter, TextField } from "../../../../ui/primitives";
import type { SettingsSection } from "../../frame/types";
import { taskError } from "../../../tasks/model";

const emptyAccount = (): MailAccountInput => ({ id: null, label: "", host: "", port: 587, tls: "starttls", username: "", from: "", password: null });

export function EmailSettings() {
  const t = useT();
  const [accounts, setAccounts] = useState<MailAccountInfo[]>([]);
  const [editing, setEditing] = useState<MailAccountInput | null>(null);
  const [deleting, setDeleting] = useState<string | null>(null);
  const [confirmed, setConfirmed] = useState(false);
  const [to, setTo] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [testing, setTesting] = useState<{ id: string; handle: OpHandle } | null>(null);
  const lock = useRef(false);
  const status = useSession(s => testing ? s.ops[testing.handle.opId] : undefined);
  const refresh = useCallback(async () => setAccounts(await mail.listAccounts()), []);
  useEffect(() => { const abort = new AbortController(); void mail.listAccounts(abort.signal).then(setAccounts).catch(e => { if (!abort.signal.aborted) setError(taskError(e)); }); return () => abort.abort(); }, []);
  const act = async (fn: () => Promise<void>) => {
    if (lock.current) return;
    lock.current = true; setBusy(true); setError(null);
    try { await fn(); } catch (e) { setError(taskError(e)); }
    finally { lock.current = false; setBusy(false); }
  };
  const testRunning = !!testing && (!status || status.state === "running");
  const result = status?.state === "done" ? status.result as MailTestResult | undefined : undefined;
  const valid = editing && editing.label.trim() && editing.host.trim() && editing.username.trim() && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(editing.from) && Number.isInteger(editing.port) && editing.port > 0 && editing.port < 65536 && (editing.tls !== "none" || confirmed);
  const closeEditor = () => { if (!lock.current) { setEditing(null); setConfirmed(false); setError(null); } };
  return <div className="os-settings-group">
    <Button onPress={() => { setEditing(emptyAccount()); setConfirmed(false); }} isDisabled={busy}>{t("email-add")}</Button>
    <Button onPress={() => void act(refresh)} isDisabled={busy}>{t("email-refresh")}</Button>
    <TextField label={t("email-test-to")} value={to} onChange={setTo} />
    <ul>{accounts.map(account => <li key={account.id}>
      <span>{account.label} — {account.host}:{account.port} ({account.tls.toUpperCase()}) · {account.from} · {t(account.hasSecret ? "email-secret-present" : "email-secret-missing")}</span>
      <Button isDisabled={busy || testRunning} onPress={() => { setConfirmed(false); setEditing({ id: account.id, label: account.label, host: account.host, port: account.port, tls: account.tls, from: account.from, username: "", password: null }); }}>{t("email-edit")}</Button>
      <Button isDisabled={busy || testRunning} onPress={() => setDeleting(account.id)}>{t("email-delete")}</Button>
      <Button isDisabled={busy || testRunning || !/^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(to)} onPress={() => void act(async () => setTesting({ id: account.id, handle: await mail.testAccount({ id: account.id, to: to.trim() }) }))}>{t("email-test")}</Button>
    </li>)}</ul>
    {testRunning && <p role="status">{t("email-test-running")}</p>}
    {result?.ok && <p role="status">{t("email-test-ok")}</p>}
    {result && !result.ok && <p role="alert">{result.error ? taskError(result.error) : t("email-test-failed")}</p>}
    {status?.state === "failed" && <p role="alert">{status.error ? taskError(status.error) : t("email-test-failed")}</p>}
    {status?.state === "cancelled" && <p role="status">{t("email-test-cancelled")}</p>}
    {error && !editing && !deleting && <p role="alert">{error}</p>}
    {editing && <Dialog title={t(editing.id ? "email-edit" : "email-add")} isOpen onOpenChange={open => { if (!open) closeEditor(); }} isDismissable={!busy}
      footer={<DialogFooter okLabel={t("email-save")} cancelLabel={t("ui-dialog-cancel")} okDisabled={busy || !valid} onCancel={closeEditor} onOk={() => void act(async () => {
        await mail.saveAccount({ account: { ...editing, password: editing.password || null } });
        // Forget the password immediately after IPC; never put it in preferences or browser storage.
        setEditing(null); setConfirmed(false); await refresh();
      })} />}>
      <fieldset disabled={busy} className="os-settings-group">
        <TextField label={t("email-label")} value={editing.label} onChange={label => setEditing({ ...editing, label })} />
        <TextField label={t("email-host")} value={editing.host} onChange={host => setEditing({ ...editing, host })} />
        <TextField type="number" label={t("email-port")} value={String(editing.port)} onChange={v => setEditing({ ...editing, port: Number(v) })} />
        <label>{t("email-tls")}<select value={editing.tls} onChange={e => { setConfirmed(false); setEditing({ ...editing, tls: e.target.value as MailAccountInput["tls"] }); }}><option value="starttls">STARTTLS</option><option value="tls">TLS</option><option value="none">{t("email-none")}</option></select></label>
        {editing.tls === "none" && <Checkbox isSelected={confirmed} onChange={setConfirmed}>{t("email-insecure-confirm")}</Checkbox>}
        <TextField label={t("email-username")} value={editing.username} onChange={username => setEditing({ ...editing, username })} description={editing.id ? t("email-username-again") : undefined} />
        <TextField type="password" label={t("email-password")} value={editing.password ?? ""} onChange={password => setEditing({ ...editing, password })} description={editing.id ? t("email-password-keep") : undefined} />
        <TextField label={t("email-from")} value={editing.from} onChange={from => setEditing({ ...editing, from })} />
      </fieldset>
      {error && <p role="alert">{error}</p>}
    </Dialog>}
    {deleting && <Dialog title={t("email-delete")} description={t("email-delete-confirm")} isOpen onOpenChange={open => { if (!open && !busy) setDeleting(null); }} isDismissable={!busy}
      footer={<DialogFooter okLabel={t("email-delete")} cancelLabel={t("ui-dialog-cancel")} okVariant="danger" okDisabled={busy} onCancel={() => { if (!busy) setDeleting(null); }} onOk={() => void act(async () => { await mail.deleteAccount({ id: deleting }); setDeleting(null); await refresh(); })} />}>{error && <p role="alert">{error}</p>}</Dialog>}
  </div>;
}

export const section: SettingsSection = { id: "email", order: 85, titleKey: "settings-section-email", Component: EmailSettings };

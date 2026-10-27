import { useEffect, useRef, useState } from "react";
import { useT } from "../../../../i18n";
import { auth } from "../../../../ipc/auth";
import { op } from "../../../../ipc/op";
import { useSession } from "../../../../state/session";
import { Button, TextField } from "../../../../ui/primitives";
import { Dialog, DialogFooter } from "../../../../ui/primitives/Dialog";
import { useConfig, useConfigValue } from "../../frame/context";
import { FieldFrame } from "../../frame/fields";
import { reportError } from "../../../notifications/controller";
import { guardDefaults, loginStatus, type FormLogin, type LoginStatus } from "./session";

export function FormLoginsPanel() {
  const t = useT();
  const { draft } = useConfig();
  const { value, set, lock } = useConfigValue<FormLogin[] | undefined>("auth.form");
  const rows = value ?? [];
  const [url, setUrl] = useState("");
  const [name, setName] = useState("");
  const [operation, setOperation] = useState<string | null>(null);
  const active = useRef<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [statuses, setStatuses] = useState<LoginStatus[]>([]);
  const state = useSession((s) => operation ? s.ops[operation] : undefined);
  useEffect(() => {
    let live = true;
    const refresh = () => void loginStatus().then((s) => { if (live) setStatuses(s); }).catch(() => undefined);
    refresh(); const timer = setInterval(refresh, 1000);
    return () => { live = false; clearInterval(timer); if (active.current) void op.cancel({ opId: active.current }).catch(() => undefined); };
  }, []);
  useEffect(() => {
    if (state?.state === "cancelled" || state?.state === "failed") {
      active.current = null; setOperation(null);
      if (state.error) reportError(state.error);
    }
  }, [state]);
  async function open(profile?: string, loginUrl = url) {
    setBusy(true);
    try { const handle = await auth.openLogin({ url: loginUrl, profile }); active.current = handle.opId; setOperation(handle.opId); }
    catch (e) { reportError(e); }
    finally { setBusy(false); }
  }
  async function cancel() {
    const id = active.current; active.current = null; setOperation(null);
    if (id) try { await op.cancel({ opId: id }); } catch (e) { reportError(e); }
  }
  async function finish() {
    if (!operation) return;
    setBusy(true);
    try {
      const profile = await auth.finishLogin({ opId: operation, saveAs: name.trim() || undefined });
      if (profile) {
        const next = await loginStatus(); setStatuses(next);
        const row = next.find((r) => r.profile === profile.name) ?? next.find((r) => r.loginUrl === url);
        // Metadata only: no browser cookies or tokens enter the draft.
        if (row) set([...rows.filter((r) => r.login_url !== row.loginUrl), { login_url: row.loginUrl, profile: profile.name, captured_at: row.capturedAt, cookie_count: row.cookies }]);
        if (!rows.some((r) => !!r.profile) && !(draft.get<string[]>("auth.session_guard") ?? []).some((p) => p.trim())) draft.set("auth.session_guard", guardDefaults);
      }
      active.current = null; setOperation(null); setUrl(""); setName("");
    } catch (e) { reportError(e); }
    finally { setBusy(false); }
  }
  return <FieldFrame path="auth.form">
    <h3>{t("form-login-title")}</h3>
    <table className="os-cfg-table" data-testid="auth-form"><thead><tr><th>{t("form-login-url")}</th><th>{t("form-login-state")}</th><th>{t("form-login-time")}</th><th>{t("form-login-cookies")}</th><th>{t("form-login-actions")}</th></tr></thead>
      <tbody>{rows.map((row) => <tr key={row.login_url}><td>{row.login_url}</td><td>{t(`form-login-${statuses.find((s) => s.profile === row.profile && s.loginUrl === row.login_url)?.state ?? "missing"}`, { count: row.cookie_count })}</td><td>{row.captured_at ?? "–"}</td><td>{row.cookie_count}</td><td>
        <Button isDisabled={lock !== null || busy || !!operation} onPress={() => { setUrl(row.login_url); void open(row.profile ?? undefined, row.login_url); }}>{t("form-login-renew")}</Button>
        <Button isDisabled={lock !== null || busy} onPress={() => set(rows.filter((r) => r !== row))}>{t("form-login-remove")}</Button>
      </td></tr>)}</tbody></table>
    <TextField label={t("form-login-url")} value={url} onChange={setUrl} isDisabled={lock !== null || !!operation} />
    <TextField label={t("form-login-name")} value={name} onChange={setName} isDisabled={lock !== null || busy} />
    <Button isDisabled={lock !== null || busy || !!operation || !/^https?:\/\//.test(url)} onPress={() => void open()}>{t("form-login-add")}</Button>
    <Dialog title={t("form-login-title")} isOpen={operation !== null} onOpenChange={(open) => { if (!open && !busy) void cancel(); }} footer={<DialogFooter okLabel={t("form-login-capture")} cancelLabel={t("form-login-cancel")} onOk={() => { if (!busy) void finish(); }} onCancel={() => { if (!busy) void cancel(); }} />}>
      <p role="status">{t("form-login-instructions")}</p>
    </Dialog>
  </FieldFrame>;
}

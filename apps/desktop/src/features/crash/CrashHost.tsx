// OUT-247: nicht-modaler Hinweis beim Start, „Bericht ansehen…“ (genau der gesendete JSON-Text), Senden nur nach
// Klick, Hilfe → „Absturzberichte…“ mit Ansehen/Senden/Löschen. Richtlinie DisableCrashReports: kein Hinweis,
// „Senden“ ausgegraut mit „Durch Administrator deaktiviert“.
import { useEffect, useId, useState } from "react";
import { useLanguageStore, useT } from "../../i18n";
import { crash } from "../../ipc/crash";
import type { CrashSendFailure } from "../../ipc/gen/CrashSendFailure";
import { registerAction } from "../../ui/actions";
import { Button } from "../../ui/primitives/Button";
import { Dialog } from "../../ui/primitives/Dialog";
import { deleteReport, dismissHint, loadOverview, openList, openView, sendHint, sendOne, useCrash, type SendState } from "./state";
import "./crash.css";

type T = ReturnType<typeof useT>;

export const MAX_NOTE = 2000;
export const MAX_EMAIL = 254;

/** Ursache eines fehlgeschlagenen Sendens als Text. */
export function failureText(t: T, failure: CrashSendFailure | null, messageKey?: string): string {
  if (messageKey) return t(messageKey);
  if (failure === null) return t("crash-failure-response");
  if (typeof failure === "object") return t(failure.http === 429 ? "crash-failure-rate" : "crash-failure-http", { status: failure.http });
  return t(`crash-failure-${failure === "notFound" ? "not-found" : failure === "alreadySent" ? "already-sent" : failure}`);
}

export function validEmail(email: string): boolean {
  const value = email.trim();
  if (!value) return true;
  return value.length <= MAX_EMAIL && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(value) && !/[\u0000-\u001f\u007f]/.test(value);
}

function kindLabel(t: T, kind: string): string {
  return t(kind === "panic" ? "crash-kind-panic" : kind === "frontend" ? "crash-kind-frontend" : kind === "unclean_exit" ? "crash-kind-unclean" : "crash-kind-other");
}

function formatTime(time: string, lang: string): string {
  const date = new Date(time);
  return Number.isNaN(date.getTime()) ? time : date.toLocaleString(lang);
}

function SendStatus({ state }: { state: SendState }) {
  const t = useT();
  if (state.kind === "sending") return <p role="status">{t("crash-sending")}</p>;
  if (state.kind === "done") return <p role="status" data-testid="crash-thanks">{t("crash-thanks", { ref: state.refs.join(", ") })}</p>;
  if (state.kind === "failed") return <p role="alert">{t("crash-send-failed", { cause: failureText(t, state.failure, state.messageKey) })}</p>;
  return null;
}

/** Nicht-modaler Hinweis im Hauptfenster. */
function Hint() {
  const t = useT();
  const { hint, hintOpen, hintSend, overview } = useCrash();
  if (!hintOpen || hint.length === 0) return null;
  const disabled = overview?.disabledByPolicy === true;
  const busy = hintSend.kind === "sending";
  return (
    <section className="os-crash-hint" role="region" aria-label={t("crash-hint-region")}>
      <p className="os-crash-hint-title">{t("crash-hint-title")}</p>
      <div aria-live="polite"><SendStatus state={hintSend} /></div>
      <div className="os-crash-actions">
        {hintSend.kind === "done" ? (
          <Button onPress={() => useCrash.setState({ hintOpen: false })}>{t("crash-close")}</Button>
        ) : (
          <>
            <Button isDisabled={busy} onPress={() => openView({ ids: hint, selected: hint[0] ?? "", fromHint: true })}>{t("crash-view")}</Button>
            <Button variant="primary" isDisabled={busy || disabled} onPress={() => void sendHint()}>
              {t(hintSend.kind === "failed" ? "crash-retry" : "crash-send")}
            </Button>
            <Button isDisabled={busy} onPress={() => void dismissHint()}>{t("crash-dont-send")}</Button>
          </>
        )}
      </div>
      {disabled && <p className="os-crash-note">{t("crash-disabled-admin")}</p>}
    </section>
  );
}

/** „Bericht ansehen…“: genau der Text, der als `report` gesendet würde; Notiz und E-Mail werden nur gesendet. */
function ReportDialog() {
  const t = useT();
  const view = useCrash((s) => s.view);
  const overview = useCrash((s) => s.overview);
  const hintSend = useCrash((s) => s.hintSend);
  const rowSend = useCrash((s) => s.rowSend);
  const [text, setText] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [note, setNote] = useState("");
  const [email, setEmail] = useState("");
  const ids = useId();
  const selected = view?.selected ?? "";
  useEffect(() => {
    if (!selected) return;
    const abort = new AbortController();
    setText(null);
    setError(null);
    crash.read({ id: selected }, abort.signal).then(
      (value) => { if (!abort.signal.aborted) setText(value); },
      () => { if (!abort.signal.aborted) setError("crash-failure-not-found"); },
    );
    return () => abort.abort();
  }, [selected]);
  if (!view) return null;
  const close = () => { useCrash.setState({ view: null }); setNote(""); setEmail(""); };
  const disabled = overview?.disabledByPolicy === true;
  const state = view.fromHint ? hintSend : rowSend[selected] ?? { kind: "idle" };
  const sent = overview?.reports.find((r) => r.id === selected)?.sent;
  const emailOk = validEmail(email);
  const busy = state.kind === "sending";
  const send = async () => {
    const result = view.fromHint ? await sendHint(note, email.trim()) : await sendOne(selected, note, email.trim());
    if (result.kind === "done") { setNote(""); setEmail(""); }
  };
  return (
    <Dialog title={t("crash-view-title")} isOpen onOpenChange={(open) => { if (!open) close(); }} size="large"
      footer={<div className="os-dialog-footer-actions">
        <Button variant="primary" isDisabled={disabled || busy || !emailOk || !!sent || text === null} onPress={() => void send()}>
          {t(state.kind === "failed" ? "crash-retry" : "crash-send")}
        </Button>
        <Button onPress={close}>{t("crash-close")}</Button>
      </div>}>
      <div className="os-crash-stack">
        {view.ids.length > 1 && (
          <label>{t("crash-view-select")}{" "}
            <select value={selected} onChange={(e) => useCrash.setState({ view: { ...view, selected: e.target.value } })}>
              {view.ids.map((id, n) => <option key={id} value={id}>{`${n + 1} / ${view.ids.length}`}</option>)}
            </select>
          </label>
        )}
        {error && <p role="alert">{t(error)}</p>}
        <label htmlFor={`${ids}-json`}>{t("crash-view-json")}</label>
        <textarea id={`${ids}-json`} className="os-crash-json" readOnly value={text ?? ""} rows={16} spellCheck={false} data-testid="crash-json" />
        <label htmlFor={`${ids}-note`}>{t("crash-view-note")}</label>
        <textarea id={`${ids}-note`} value={note} maxLength={MAX_NOTE} rows={3} onChange={(e) => setNote(e.target.value)} aria-describedby={`${ids}-only-sent`} />
        <label htmlFor={`${ids}-email`}>{t("crash-view-email")}</label>
        <input id={`${ids}-email`} type="email" value={email} maxLength={MAX_EMAIL} onChange={(e) => setEmail(e.target.value)}
          aria-invalid={!emailOk} aria-describedby={`${ids}-only-sent`} />
        {!emailOk && <p role="alert">{t("crash-email-invalid")}</p>}
        <p id={`${ids}-only-sent`} className="os-crash-note">{t("crash-view-only-sent")}</p>
        {disabled && <p className="os-crash-note">{t("crash-disabled-admin")}</p>}
        <div aria-live="polite"><SendStatus state={state} /></div>
      </div>
    </Dialog>
  );
}

/** Hilfe → „Absturzberichte…“. */
function ListDialog() {
  const t = useT();
  const lang = useLanguageStore((s) => s.lang);
  const { listOpen, overview, rowSend } = useCrash();
  const [error, setError] = useState<string | null>(null);
  if (!listOpen) return null;
  const close = () => useCrash.setState({ listOpen: false });
  const disabled = overview?.disabledByPolicy === true;
  const reports = overview?.reports ?? [];
  return (
    <Dialog title={t("crash-list-title")} isOpen onOpenChange={(open) => { if (!open) close(); }} size="large"
      footer={<div className="os-dialog-footer-actions"><Button onPress={close}>{t("crash-close")}</Button></div>}>
      <div className="os-crash-stack">
        <p>{t("crash-list-intro")}</p>
        {disabled && <p className="os-crash-note">{t("crash-disabled-admin")}</p>}
        {error && <p role="alert">{error}</p>}
        {reports.length === 0 ? <p>{t("crash-list-empty")}</p> : (
          <table className="os-crash-table">
            <thead><tr>
              <th scope="col">{t("crash-col-time")}</th><th scope="col">{t("crash-col-kind")}</th>
              <th scope="col">{t("crash-col-sent")}</th><th scope="col">{t("crash-col-ref")}</th><th scope="col">{t("crash-col-actions")}</th>
            </tr></thead>
            <tbody>
              {reports.map((r) => {
                const state = rowSend[r.id] ?? { kind: "idle" };
                return <tr key={r.id} data-crash-id={r.id}>
                  <td>{formatTime(r.time, lang)}</td>
                  <td>{kindLabel(t, r.kind)}{r.process === "cli" ? ` (${t("crash-process-cli")})` : ""}</td>
                  <td>{t(r.sent ? "crash-yes" : "crash-no")}</td>
                  <td>{r.serverRef ?? "—"}</td>
                  <td>
                    <div className="os-crash-actions">
                      <Button onPress={() => openView({ ids: [r.id], selected: r.id, fromHint: false })}>{t("crash-row-view")}</Button>
                      <Button isDisabled={disabled || !!r.sent || state.kind === "sending"} onPress={() => void sendOne(r.id)}>
                        {t(state.kind === "failed" ? "crash-retry" : "crash-send")}
                      </Button>
                      <Button onPress={() => { setError(null); void deleteReport(r.id).catch(() => setError(t("crash-delete-failed"))); }}>{t("crash-row-delete")}</Button>
                    </div>
                    <SendStatus state={state} />
                  </td>
                </tr>;
              })}
            </tbody>
          </table>
        )}
      </div>
    </Dialog>
  );
}

/** Hauptfenster: lädt beim Start die Übersicht (ohne Netzwerkzugriff) und registriert Hilfe → „Absturzberichte…“. */
export function CrashHost() {
  useEffect(() => {
    const abort = new AbortController();
    void loadOverview(abort.signal);
    const off = registerAction("help.crashReports", () => openList());
    return () => { abort.abort(); off(); };
  }, []);
  return <>
    <Hint />
    <ReportDialog />
    <ListDialog />
  </>;
}

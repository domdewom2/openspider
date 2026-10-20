// Dialog „API-Zugriff“ (INTEG-001): alle Integrationen mit jederzeit sichtbarem Status; je Integration Konto/Verbindung,
// Metriken und Erweitert. Verbinden/Trennen wirken sofort, Einstellungen erst mit OK (Rückfrage nach INTEG-010).
import { useEffect, useState } from "react";
import { i18n, currentLanguage, useT } from "../../i18n";
import { describeError, toCmdError } from "../../ipc/errors";
import type { CmdError } from "../../ipc/gen/CmdError";
import type { JsonValue } from "../../ipc/gen/serde_json/JsonValue";
import { Banner, Button, Dialog, DialogFooter, Tabs } from "../../ui/primitives";
import { SourcePanel } from "./SourcePanel";
import { DIALOG_KINDS, type DialogKind } from "./sources";
import { loadSettings, saveSettings, setIn, type Values } from "./settings";
import { statusText } from "./status";
import { closeApiAccess, useIntegrations } from "./store";
import { useApiSources } from "./useApiSources";

const has = (key: string) => i18n.has(currentLanguage(), key);

interface Draft {
  values: Values;
  dirty: boolean;
}

export function ApiAccessDialog() {
  const dialog = useIntegrations((s) => s.dialog);
  return dialog ? <ApiAccessDialogBody initial={dialog.kind} /> : null;
}

function ApiAccessDialogBody({ initial }: { initial: DialogKind }) {
  const t = useT();
  const conn = useApiSources();
  const [kind, setKind] = useState<DialogKind>(initial);
  const [drafts, setDrafts] = useState<Partial<Record<DialogKind, Draft>>>({});
  const [failure, setFailure] = useState<CmdError | null>(null);
  const [confirm, setConfirm] = useState<DialogKind[]>([]);
  const [saving, setSaving] = useState(false);
  useEffect(() => setKind(initial), [initial]);

  useEffect(() => {
    if (drafts[kind]) return;
    let live = true;
    loadSettings(kind).then(
      (values) => live && setDrafts((d) => (d[kind] ? d : { ...d, [kind]: { values, dirty: false } })),
      (e) => live && setFailure(toCmdError(e)),
    );
    return () => {
      live = false;
    };
  }, [kind, drafts]);

  const change = (target: DialogKind) => (path: readonly string[], value: JsonValue) =>
    setDrafts((d) => {
      const cur = d[target];
      return cur ? { ...d, [target]: { values: setIn(cur.values, path, value), dirty: true } } : d;
    });

  /** Speichert alle geänderten Quellen; Quellen mit vorhandenen Daten warten auf die Entscheidung des Nutzers. */
  const save = async (decisions: Partial<Record<DialogKind, "keep" | "reload">> = {}) => {
    setSaving(true);
    setFailure(null);
    const ask: DialogKind[] = [];
    try {
      for (const k of DIALOG_KINDS) {
        const draft = drafts[k];
        if (!draft?.dirty) continue;
        const result = await saveSettings(k, draft.values, decisions[k] ?? "ask");
        if (result.confirmationRequired) ask.push(k);
        else setDrafts((d) => ({ ...d, [k]: { values: draft.values, dirty: false } }));
      }
    } catch (e) {
      setFailure(toCmdError(e));
      setSaving(false);
      return;
    }
    setSaving(false);
    if (ask.length > 0) setConfirm(ask);
    else closeApiAccess();
  };

  const decide = (decision: "keep" | "reload") => void save(Object.fromEntries(confirm.map((k) => [k, decision])));
  const navStatus = (k: DialogKind) =>
    statusText(t, k, { state: conn.stateOf(k), parts: [conn.sources?.find((s) => s.kind === k)?.account], error: conn.errorOf(k) }, has);

  return (
    <Dialog
      title={t("integ-title")}
      description={t("integ-description")}
      size="large"
      isOpen
      onOpenChange={(open) => !open && closeApiAccess()}
      footer={
        <DialogFooter
          okLabel={t("ui-dialog-ok")}
          cancelLabel={t("ui-dialog-cancel")}
          okDisabled={saving || confirm.length > 0}
          onOk={() => void save()}
          onCancel={closeApiAccess}
        />
      }
    >
      {conn.error && <Banner kind="error">{describeError(t, conn.error, has).text}</Banner>}
      {failure && <Banner kind="error" onDismiss={() => setFailure(null)} dismissLabel={t("ui-dialog-close")}>{describeError(t, failure, has).text}</Banner>}
      {confirm.length > 0 && (
        <Banner
          kind="warning"
          action={
            <span className="os-integ-actions">
              <Button variant="danger" onPress={() => decide("reload")}>{t("integ-invalidate-reload")}</Button>
              <Button onPress={() => decide("keep")}>{t("integ-invalidate-keep")}</Button>
            </span>
          }
        >
          {t("integ-invalidate-question", { sources: confirm.map((k) => t(`api-${k}`)).join(", ") })}
        </Banner>
      )}
      <Tabs
        orientation="vertical"
        label={t("integ-list")}
        selected={kind}
        onSelect={(id) => setKind(id as DialogKind)}
        className="os-integ-layout"
        tabs={DIALOG_KINDS.map((k) => ({
          id: k,
          label: (
            <span className="os-integ-entry">
              <span className="os-integ-entry-name">{t(`api-${k}`)}</span>
              <span className="os-integ-entry-status" data-state={conn.stateOf(k)}>{navStatus(k)}</span>
            </span>
          ),
          content:
            k === kind ? (
              <SourcePanel kind={k} info={conn.sources?.find((s) => s.kind === k)} conn={conn} values={drafts[k]?.values ?? null} onChange={change(k)} />
            ) : null,
        }))}
      />
    </Dialog>
  );
}

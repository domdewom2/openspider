// Kopfleisten-Variante des APIs-Modus (INTEG-012): URL-Liste (Eingabe oder Datei), Vorschau, verbundene Integrationen,
// Start ohne Abruf der Zielseiten. Ohne verbundene Integration erscheint ein Hinweis statt eines leeren Laufs.
import { useEffect, useId, useState } from "react";
import { i18n, currentLanguage, useT } from "../../i18n";
import { api } from "../../ipc/api";
import { crawl } from "../../ipc/crawl";
import { describeError, toCmdError } from "../../ipc/errors";
import type { ListPreview } from "../../ipc/gen/ListPreview";
import { Banner, Button } from "../../ui/primitives";
import { connectedSources, parseUrlLines, startApisMode } from "./apisMode";
import { openApiAccess, useIntegrations } from "./store";
import { useApiSources } from "./useApiSources";

const has = (key: string) => i18n.has(currentLanguage(), key);

export function ApiModeBar() {
  const t = useT();
  const id = useId();
  const conn = useApiSources();
  const [text, setText] = useState("");
  const [preview, setPreview] = useState<ListPreview | null>(null);
  const [message, setMessage] = useState<{ kind: "warning" | "error" | "info"; text: string } | null>(null);
  const [starting, setStarting] = useState(false);
  const connected = connectedSources(conn.sources ?? []);
  const list = parseUrlLines(text);
  const dialogOpen = useIntegrations((s) => s.dialog !== null);
  const { reload } = conn;
  useEffect(() => {
    if (!dialogOpen) void reload();
  }, [dialogOpen, reload]);

  useEffect(() => {
    if (list.items.length === 0) {
      setPreview(null);
      return;
    }
    const ctrl = new AbortController();
    const timer = setTimeout(() => {
      crawl.parseList({ kind: "text", text }, ctrl.signal).then(setPreview, () => setPreview(null));
    }, 250);
    return () => {
      clearTimeout(timer);
      ctrl.abort();
    };
  }, [text, list.items.length]);

  const start = async () => {
    setMessage(null);
    setStarting(true);
    try {
      // Frischer Stand: im Dialog kann sich eine Verbindung seit dem Öffnen der Leiste geändert haben.
      const result = await startApisMode(text, await api.list());
      if (!result.started) setMessage({ kind: "warning", text: t(result.reason === "no-integration" ? "integ-apimode-no-integration" : "integ-apimode-empty") });
      else setMessage({ kind: "info", text: t("integ-apimode-started", { count: list.items.length }) });
    } catch (e) {
      setMessage({ kind: "error", text: describeError(t, toCmdError(e), has).text });
    } finally {
      setStarting(false);
    }
  };

  const load = async (file: File | undefined) => {
    if (!file) return;
    setText(await file.text());
  };

  return (
    <section className="os-integ-apimode" aria-label={t("integ-apimode")}>
      <div className="os-field">
        <label className="os-label" htmlFor={`${id}-urls`}>
          {t("integ-apimode-urls")}
        </label>
        <textarea id={`${id}-urls`} className="os-input os-integ-urls" rows={3} value={text} onChange={(e) => setText(e.target.value)} spellCheck={false} />
      </div>
      <div className="os-integ-actions">
        <label className="os-btn os-btn-secondary os-integ-file">
          {t("integ-apimode-file")}
          <input type="file" accept=".txt,.csv,text/plain,text/csv" onChange={(e) => void load(e.target.files?.[0])} />
        </label>
        <Button variant="primary" isDisabled={starting} onPress={() => void start()}>
          {t("integ-apimode-start")}
        </Button>
        <Button onPress={() => openApiAccess()}>{t("integ-open")}</Button>
      </div>
      <p className="os-hint" role="status">
        {preview
          ? t("integ-apimode-preview", { valid: preview.valid, invalid: preview.invalid, duplicates: preview.duplicates })
          : t("integ-apimode-count", { count: list.items.length, duplicates: list.duplicates })}
      </p>
      <p className="os-hint">
        {connected.length > 0
          ? t("integ-apimode-sources", { sources: connected.map((s) => t(s.labelKey)).join(", ") })
          : t("integ-apimode-none-connected")}
      </p>
      {message && (
        <Banner kind={message.kind} onDismiss={() => setMessage(null)} dismissLabel={t("ui-dialog-close")}>
          {message.text}
        </Banner>
      )}
    </section>
  );
}

// Gemeinsame Bausteine der Fach-Detailtabs (M2-U21): Konfigurationsschalter, Leerzustände, Operationsergebnisse, Dateiablage.
import { useEffect, useState } from "react";
import { analysis } from "../../../ipc/analysis";
import { config } from "../../../ipc/config";
import { eventBus } from "../../../ipc/events";
import type { EmptyAction } from "../../../ipc/gen/EmptyAction";
import type { EmptyState as EmptyStateDto } from "../../../ipc/gen/EmptyState";
import type { OpStatus } from "../../../ipc/gen/OpStatus";
import { openSettingsWindow } from "../../../app/windows";
import { useSession } from "../../../state/session";
import { EmptyState } from "../../../ui/primitives/EmptyState";
import { pickEmptyState } from "../../../ui/emptyState/codes";
import type { DetailSelection } from "../frame/types";
import "./fach.css";

/** Für die Fach-Tabs relevante Schalter der Crawl-Konfiguration (Schlüssel nach `osconfig.schema.json`). */
export interface FachConfig {
  loaded: boolean;
  error?: unknown;
  rendering: boolean;
  screenshots: boolean;
  jsErrors: boolean;
  storeHtml: boolean;
  storeRenderedHtml: boolean;
  accessibility: boolean;
  structuredData: { jsonLd: boolean; microdata: boolean; rdfa: boolean };
  exactDuplicates: boolean;
  nearDuplicates: boolean;
  spelling: boolean;
  grammar: boolean;
}
const OFF: Omit<FachConfig, "loaded"> = {
  rendering: false, screenshots: true, jsErrors: false, storeHtml: false, storeRenderedHtml: false, accessibility: false,
  structuredData: { jsonLd: false, microdata: false, rdfa: false }, exactDuplicates: true, nearDuplicates: false, spelling: false, grammar: false,
};
type Json = Record<string, unknown>;
const obj = (value: unknown): Json => value && typeof value === "object" && !Array.isArray(value) ? value as Json : {};
export function parseFachConfig(raw: unknown): Omit<FachConfig, "loaded"> {
  const cfg = obj(raw); const rendering = obj(cfg.rendering); const extraction = obj(cfg.extraction); const sd = obj(extraction.structured_data);
  const duplicates = obj(cfg.duplicates);
  return {
    rendering: rendering.mode === "javascript", screenshots: obj(rendering.screenshots).enabled !== false, jsErrors: rendering.js_error_reporting !== false,
    storeHtml: extraction.store_html === true, storeRenderedHtml: extraction.store_rendered_html === true, accessibility: extraction.accessibility === true,
    structuredData: { jsonLd: sd.json_ld === true, microdata: sd.microdata === true, rdfa: sd.rdfa === true },
    exactDuplicates: duplicates.exact_enabled !== false, nearDuplicates: duplicates.near_enabled === true,
    spelling: obj(cfg.spelling).enabled === true, grammar: obj(cfg.grammar).enabled === true,
  };
}
/** Lädt die Konfiguration einmal je Crawl und nach jeder gemeldeten Änderung neu; Antworten alter Anfragen werden verworfen. */
export function useFachConfig(crawlId?: string): FachConfig {
  const [state, setState] = useState<FachConfig>({ loaded: false, ...OFF });
  const [version, setVersion] = useState(0);
  useEffect(() => eventBus.on("settingsChanged", () => setVersion(v => v + 1)), []);
  useEffect(() => {
    const controller = new AbortController();
    void config.get(controller.signal).then(dto => { if (!controller.signal.aborted) setState({ loaded: true, ...parseFachConfig(dto.config) }); })
      .catch(error => { if (!controller.signal.aborted) setState(previous => ({ ...previous, loaded: true, error })); });
    return () => controller.abort();
  }, [crawlId, version]);
  return state;
}

export const focusOf = (selection: DetailSelection): number => selection.focus ?? selection.urlIds[0]!;

/** Rangfolge nach UI-016: bei mehreren Ursachen gilt der Zustand mit dem kleinsten Rang. */
export function gate(...candidates: (EmptyStateDto | false | null | undefined)[]): EmptyStateDto | null {
  return pickEmptyState(candidates.filter((c): c is EmptyStateDto => !!c));
}
export const needsRendering = (): EmptyStateDto => ({ code: "NEEDS_JS_RENDERING", action: { kind: "openSetting", anchor: "rendering.mode" } });
export const featureDisabled = (key: string, anchor = key): EmptyStateDto => ({ code: "FEATURE_DISABLED", arg: key, action: { kind: "openSetting", anchor } });

/** Leerzustand mit Aktionen: Einstellung öffnen, Crawl-Analyse für den genannten Teilschritt starten. */
export function FachEmpty({ state, onError }: { state: EmptyStateDto; onError?: (error: unknown) => void }) {
  return <EmptyState state={state} onAction={(action: EmptyAction) => {
    if (action.kind === "openSetting") void openSettingsWindow(action.anchor);
    if (action.kind === "startAnalysis") void analysis.start({ steps: state.arg ? [state.arg] : null }).catch(error => onError?.(error));
  }} />;
}

/** Zustand einer lang laufenden Operation, wie ihn die Ticks der Sitzung melden. */
export function useOpStatus(opId: string | undefined): OpStatus | undefined {
  return useSession(s => opId ? s.ops[opId] : undefined);
}

/** Die Dateiablage im Browser-Kontext: Der Kern bietet keinen Schreibbefehl für Anzeigeinhalte (Auflage an K09). */
export function saveFile(name: string, data: BlobPart, type: string): void {
  const url = URL.createObjectURL(new Blob([data], { type }));
  const link = document.createElement("a");
  link.href = url; link.download = name; link.rel = "noopener";
  document.body.append(link); link.click(); link.remove();
  setTimeout(() => URL.revokeObjectURL(url), 10_000);
}
export function copyText(text: string, onError: (error: unknown) => void): void {
  void navigator.clipboard.writeText(text).catch(onError);
}
/** Dateiname aus einer Adresse: Sonderzeichen werden ersetzt (UI-039). */
export function fileStem(address: string): string {
  return address.replace(/^[a-z]+:\/\//i, "").replace(/[^A-Za-z0-9._-]+/g, "_").replace(/^_+|_+$/g, "").slice(0, 120) || "page";
}

// Fensterweiter Zustand des Integrationen-Features: offener API-Zugriffsdialog (mit gewählter Quelle) und APIs-Modus.
import { create } from "zustand";
import type { ApiKind } from "../../ipc/gen/ApiKind";
import { DIALOG_KINDS, isDialogKind, type DialogKind } from "./sources";

export interface IntegrationsState {
  /** Offener Dialog „API-Zugriff“ und darin gewählte Quelle; `null` = geschlossen. */
  dialog: { kind: DialogKind } | null;
  /** APIs-Modus der Kopfleiste aktiv (INTEG-012). */
  apiMode: boolean;
  /** Start-URL des aktuellen Crawls, falls die Kopfleiste sie meldet (Property-Hinweis INTEG-110). */
  crawlUrl: string | null;
}

export const useIntegrations = create<IntegrationsState>(() => ({ dialog: null, apiMode: false, crawlUrl: null }));

/** Öffnet den API-Zugriffsdialog, auf Wunsch direkt bei einer Quelle (Zahnrad je Quelle, UI-070). */
export function openApiAccess(kind?: ApiKind): void {
  const target = kind && isDialogKind(kind) ? kind : (useIntegrations.getState().dialog?.kind ?? DIALOG_KINDS[0]);
  useIntegrations.setState({ dialog: { kind: target } });
}

export function closeApiAccess(): void {
  useIntegrations.setState({ dialog: null });
}

/** Die Kopfleiste (U03) schaltet beim Moduswechsel hierüber; `true` zeigt die APIs-Variante. */
export function setApiMode(active: boolean): void {
  useIntegrations.setState({ apiMode: active });
}

/** Die Kopfleiste meldet die Start-URL des Crawls (für den Hinweis „Property passt nicht zur Crawl-Domain“). */
export function setCrawlUrl(url: string | null): void {
  useIntegrations.setState({ crawlUrl: url });
}

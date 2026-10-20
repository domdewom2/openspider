import { createContext, useContext, type ReactNode } from "react";
import type { TFn } from "../../../i18n";
import { currentLanguage, i18n } from "../../../i18n";
import type { ValidationIssue } from "../../../ipc/gen/ValidationIssue";
import type { ConfigDraft } from "./draft";
import { defaultResolver, issuesFor, lockReason, type LockReason, type RunMode } from "./model";

export interface ConfigContextValue {
  draft: ConfigDraft;
  runMode: RunMode;
  /** Übergibt den Crawl-Zustand „pausiert“ (Schaltfläche „Pausieren und bearbeiten“). */
  pauseAndEdit(): Promise<void>;
  pausing: boolean;
  /** Wechselt zu einer Seite (Sprungziel aus Suche oder Abweichungen). */
  goto(anchor: string): void;
  /** Zuletzt geladenes Profil (Anzeige in der Titelzeile); `null` = Stand der Sitzung. */
  profileName: string | null;
  /** Lädt ein Profil als Entwurf (gesperrt während „Läuft“); das Ergebnis erscheint als Meldung im Dialog. */
  loadProfile(name: string): Promise<boolean>;
  /** Meldet ein geändertes Profilangebot (gespeichert, gelöscht, Standard gesetzt) an die Titelzeile. */
  profilesRevision: number;
  bumpProfiles(): void;
}

const Ctx = createContext<ConfigContextValue | null>(null);

export function ConfigProvider({ value, children }: { value: ConfigContextValue; children: ReactNode }) {
  return <Ctx.Provider value={value}>{children}</Ctx.Provider>;
}

export function useConfig(): ConfigContextValue {
  const v = useContext(Ctx);
  if (!v) throw new Error("useConfig außerhalb des Konfigurationsdialogs");
  return v;
}

/** Zugriff auf einen Wert, seine Sperre und seine Prüfmeldungen. */
export function useConfigValue<T = unknown>(path: string): {
  value: T;
  set(value: T): void;
  lock: LockReason | null;
  issues: ValidationIssue[];
  standard: unknown;
} {
  const { draft, runMode } = useConfig();
  const standard = defaultResolver(draft.saved, draft.defaultRows)(path);
  return {
    value: draft.get<T>(path),
    set: (v) => draft.set(path, v),
    lock: lockReason(path, runMode),
    issues: issuesFor(draft.issues, path),
    standard,
  };
}

/** Text zu einer Kernmeldung: eigene Übersetzung der bekannten `err-*`-Schlüssel, sonst Detail oder der Schlüssel selbst. */
export function messageText(t: TFn, key: string, args: Record<string, unknown> = {}, detail?: string | null): string {
  const own = `config-msg-${key.replace(/^err-/, "")}`;
  const lang = currentLanguage();
  if (i18n.has(lang, own) || i18n.has("en", own)) return t(own, { ...args, detail: detail ?? "" });
  return detail ?? t("config-msg-unknown", { key });
}

export function issueText(t: TFn, issue: Pick<ValidationIssue, "messageKey" | "args">): string {
  const reason = typeof issue.args.reason === "string" ? issue.args.reason : undefined;
  return messageText(t, issue.messageKey, issue.args as Record<string, unknown>, reason);
}

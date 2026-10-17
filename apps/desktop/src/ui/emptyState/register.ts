// Voraussetzungsregister (UI-016): je Top-Tab, Filter, Detail-Tab und Seitenleisten-Tab eine – auch leere – Liste von
// Bedingungen. Die Daten liefert der Kern (`core/ui/prerequisites.toml`); hier steht die Auswertung und die CI-Prüfung der
// Abdeckung.
import { pickEmptyState, type EmptyAction, type EmptyCode, type EmptyState } from "./codes";

export type RegisterKind = "topTab" | "filter" | "detailTab" | "sidebarTab";

export interface Prerequisite {
  code: EmptyCode;
  arg?: string;
  action?: EmptyAction;
}

export interface RegisterEntry {
  kind: RegisterKind;
  /** Tab-Name, `Tab:Filter` bzw. Kennung des Detail-/Seitenleisten-Tabs. */
  id: string;
  conditions: readonly Prerequisite[];
}

export const registerKey = (kind: RegisterKind, id: string) => `${kind}|${id.toLowerCase()}`;

/** CI-Test: jeder registrierte Filter/Tab braucht einen Eintrag (auch mit leerer Liste); liefert die fehlenden. */
export function findMissingRegisterEntries(
  register: readonly RegisterEntry[],
  required: readonly { kind: RegisterKind; id: string }[],
): string[] {
  const have = new Set(register.map((e) => registerKey(e.kind, e.id)));
  return required.filter((r) => !have.has(registerKey(r.kind, r.id))).map((r) => `${r.kind}:${r.id}`);
}

/**
 * Wertet einen Registereintrag aus: `unmet` sagt, ob eine Bedingung gerade verletzt ist. Ergebnis ist der Zustand mit dem
 * kleinsten Rang oder `null`, wenn alle Voraussetzungen erfüllt sind.
 */
export function evaluatePrerequisites(entry: RegisterEntry, unmet: (p: Prerequisite) => boolean): EmptyState | null {
  const active = entry.conditions.filter(unmet).map((p) => ({ code: p.code, arg: p.arg ?? null, action: p.action ?? null }));
  return pickEmptyState(active);
}

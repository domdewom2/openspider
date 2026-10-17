// Register der Menübeiträge: jedes Feature hängt `features/<f>/menu.ts` an; hier nur Sammeln und Beobachten.
import type { MenuContribution } from "./types";

const contributions = new Map<string, readonly MenuContribution[]>();
const listeners = new Set<() => void>();

/** Setzt (ersetzt) die Beiträge eines Besitzers, z. B. `register("features/help", [...])`; liefert die Rücknahme. */
export function registerMenuContributions(owner: string, list: readonly MenuContribution[]): () => void {
  contributions.set(owner, list);
  for (const l of listeners) l();
  return () => {
    contributions.delete(owner);
    for (const l of listeners) l();
  };
}

export function allMenuContributions(): MenuContribution[] {
  return [...contributions.values()].flat();
}

export function onMenuContributionsChanged(cb: () => void): () => void {
  listeners.add(cb);
  return () => listeners.delete(cb);
}

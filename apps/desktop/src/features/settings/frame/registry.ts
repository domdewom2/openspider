import type { FC } from "react";
import { KNOWN_SECTIONS, sectionTitleKey, type SettingsSection } from "./types";

const modules = import.meta.glob<{ section?: SettingsSection }>("../sections/*/index.tsx", { eager: true });

export interface ResolvedSection extends Omit<SettingsSection, "Component"> {
  Component: FC | null;
}

/** Alle Abschnitte: gefundene Module plus Platzhalter für bekannte, noch nicht gelieferte (Component = null). */
export function resolveSections(found: readonly SettingsSection[] = Object.values(modules).flatMap((m) => (m.section ? [m.section] : []))): ResolvedSection[] {
  const byId = new Map<string, ResolvedSection>();
  for (const k of KNOWN_SECTIONS) byId.set(k.id, { id: k.id, order: k.order, titleKey: sectionTitleKey(k.id), Component: null });
  for (const s of found) byId.set(s.id, s);
  return [...byId.values()].sort((a, b) => a.order - b.order);
}

/** `appearance.zoom` → Abschnitt `appearance`. */
export function sectionOfAnchor(anchor: string): string {
  return anchor.split(".")[0] ?? anchor;
}

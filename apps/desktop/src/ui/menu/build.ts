// Menüaufbau aus Beiträgen (UI-002, F-5): reine Funktion Beiträge + Plattform + Sprache + Zustand → Modell.
// Windows: kein Bearbeiten-Menü und keine Accelerators für Strg+C/V/X/A/Z (Kürzel nur als Beschriftung); macOS: „Bearbeiten“
// direkt nach „Datei“ mit den vordefinierten Systemeinträgen, „Fenster“ vor „Hilfe“.
import type { TFn } from "../../i18n/core";
import { canonicalCombo, formatShortcut, isEditingCombo, parseCombo, shortcutById, type ActionId } from "../shortcuts";
import type { Platform } from "../tokens/tokens";
import {
  MENU_ORDER,
  type MenuContext,
  type MenuContribution,
  type MenuItemSpec,
  type MenuModel,
  type ResolvedItem,
  type ResolvedMenu,
  type TopMenuId,
} from "./types";

const NATIVE_KEYS: Record<string, string> = {
  Left: "ArrowLeft",
  Right: "ArrowRight",
  Up: "ArrowUp",
  Down: "ArrowDown",
  Enter: "Enter",
  Esc: "Escape",
  Del: "Delete",
  Backspace: "Backspace",
  Space: "Space",
  ContextMenu: "ContextMenu",
};

/** `Mod+Shift+A` → `CmdOrCtrl+Shift+A` (Tauri/muda). */
export function toNativeAccelerator(combo: string, platform: Platform): string {
  const c = parseCombo(combo, platform);
  const mods: string[] = [];
  if (c.ctrl && c.cmd) mods.push("Cmd", "Ctrl");
  else if (c.cmd) mods.push("CmdOrCtrl");
  else if (c.ctrl) mods.push("CmdOrCtrl");
  if (c.alt) mods.push("Alt");
  if (c.shift) mods.push("Shift");
  const key = NATIVE_KEYS[c.key] ?? c.key;
  return [...mods, key].join("+");
}

export interface BuildOptions {
  platform: Platform;
  lang: string;
  t: TFn;
  ctx: MenuContext;
  /** Native Accelerators setzen (nur in der Tauri-App; im Browser/E2E-A nur Beschriftung). */
  native: boolean;
}

function sortContributions(list: readonly MenuContribution[]): MenuContribution[] {
  // Gruppen bleiben zusammen: Reihenfolge der Gruppen nach kleinster `order`, darin nach `order`.
  const firstOrder = new Map<string, number>();
  for (const c of list) firstOrder.set(c.group, Math.min(firstOrder.get(c.group) ?? Infinity, c.order));
  return [...list].sort((a, b) => (firstOrder.get(a.group) ?? 0) - (firstOrder.get(b.group) ?? 0) || a.group.localeCompare(b.group) || a.order - b.order);
}

function resolveItem(spec: MenuItemSpec, o: BuildOptions, native: Set<ActionId>): ResolvedItem | null {
  if (spec.platforms && !spec.platforms.includes(o.platform)) return null;
  if (spec.separator) return { id: spec.id, kind: "separator", label: "", enabled: false };
  if (spec.predefined) {
    // Systemeinträge gibt es nur, wo das Betriebssystem sie liefert (macOS); Windows erhält sie nicht.
    return { id: spec.id, kind: "predefined", predefined: spec.predefined, label: spec.labelKey ? o.t(spec.labelKey) : "", enabled: true };
  }
  const label = spec.labelKey ? o.t(spec.labelKey, undefined, { fallback: spec.label }) : spec.label ?? spec.id;
  const enabled = spec.enabled ? spec.enabled(o.ctx) : true;
  const shortcutId = spec.shortcut ?? spec.action;
  const sc = shortcutId ? shortcutById(shortcutId) : undefined;
  const base: ResolvedItem = {
    id: spec.id,
    kind: spec.items ? "submenu" : spec.checked ? "check" : "item",
    label,
    enabled,
    ...(spec.showDisabledReason ? { showDisabledReason: true } : {}),
    ...(spec.action ? { action: spec.action } : {}),
    ...(!enabled && spec.disabledReasonKey ? { disabledReason: o.t(spec.disabledReasonKey) } : {}),
  };
  if (spec.literalLabel !== undefined) base.label = spec.literalLabel;
  if (spec.checked) base.checked = spec.checked(o.ctx);
  if (sc) {
    const combo = sc.keys[o.platform];
    base.shortcutText = formatShortcut(combo, o.platform, o.lang);
    if (spec.nativeAccelerator === false) base.shortcutInLabel = true;
    if (spec.nativeAccelerator !== false && o.native && !isEditingCombo(canonicalCombo(combo, o.platform))) {
      base.accelerator = toNativeAccelerator(combo, o.platform);
      native.add(sc.id);
    }
  }
  if (spec.items) {
    base.items = resolveItems(spec.items, o, native);
  }
  return base;
}

function resolveItems(specs: readonly MenuItemSpec[], o: BuildOptions, native: Set<ActionId>): ResolvedItem[] {
  const out: ResolvedItem[] = [];
  // Gleiche Eintrags-ID in mehreren Beiträgen: der später registrierte Beitrag ersetzt den früheren; mit `override`
  // bleibt die Stelle des ersten Vorkommens.
  const firstIndex = new Map<string, number>();
  const lastIndex = new Map<string, number>();
  specs.forEach((sp, i) => {
    if (sp.separator) return;
    if (!firstIndex.has(sp.id)) firstIndex.set(sp.id, i);
    lastIndex.set(sp.id, i);
  });
  for (const [i, listed] of specs.entries()) {
    let spec = listed;
    if (!listed.separator) {
      const winner = specs[lastIndex.get(listed.id)!]!;
      if (i !== (winner.override ? firstIndex.get(listed.id)! : lastIndex.get(listed.id)!)) continue;
      spec = winner;
    }
    const item = resolveItem(spec, o, native);
    if (!item) continue;
    // keine doppelten/führenden Trenner
    if (item.kind === "separator" && (out.length === 0 || out[out.length - 1]?.kind === "separator")) continue;
    out.push(item);
  }
  while (out.length > 0 && out[out.length - 1]?.kind === "separator") out.pop();
  return out;
}

export function menuLabelKey(id: TopMenuId): string {
  return "menu-" + id.replace(/([A-Z])/g, "-$1").toLowerCase();
}

export function buildMenuModel(contributions: readonly MenuContribution[], o: BuildOptions): MenuModel {
  const native = new Set<ActionId>();
  const menus: ResolvedMenu[] = [];
  for (const id of MENU_ORDER[o.platform]) {
    const mine = sortContributions(contributions.filter((c) => c.menu === id));
    const specs: MenuItemSpec[] = [];
    let lastGroup: string | null = null;
    for (const c of mine) {
      if (lastGroup !== null && c.group !== lastGroup) specs.push({ id: `sep.${id}.${c.group}`, separator: true });
      lastGroup = c.group;
      specs.push(...c.items);
    }
    const items = resolveItems(specs, o, native);
    if (items.length === 0) continue;
    menus.push({ id, label: o.t(menuLabelKey(id)), items });
  }
  return { platform: o.platform, lang: o.lang, menus, nativeAccelerators: [...native] };
}

/** Alle Aktionen eines Modells (für Tests: jeder Eintrag mit Kürzel zeigt es in Plattformschreibweise). */
export function flattenItems(items: readonly ResolvedItem[]): ResolvedItem[] {
  return items.flatMap((i) => [i, ...(i.items ? flattenItems(i.items) : [])]);
}

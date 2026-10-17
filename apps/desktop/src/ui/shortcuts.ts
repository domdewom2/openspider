// Einziges Kürzelregister (UI-113): Menüanzeige, „Hilfe → Tastaturkürzel“ und der Tastatur-Dispatcher lesen daraus.
// `Mod` = Cmd (macOS) bzw. Strg (Windows/Linux); Tastennamen: Enter, Esc, Left/Right/Up/Down, Del, Backspace, F1 … F12,
// ContextMenu, einzelne Zeichen (`,` `=` `-` `[` `]`), Buchstaben und Ziffern in Großschrift.
import type { Platform } from "./tokens/tokens";

export type Scope = "global" | "table" | "field" | "dialog";

export type ActionId = string;

export interface Shortcut {
  id: ActionId;
  keys: { mac: string; win: string };
  /** Zusätzliche gleichwertige Belegungen (z. B. Backspace für Entfernen). */
  aliases?: { mac?: readonly string[]; win?: readonly string[] };
  scope: Scope;
  /** Fluent-Schlüssel der Beschriftung (`shortcut-<id>`). */
  labelKey: string;
  /** Kern (P0) der Tabelle UI-113; alle übrigen Zeilen sind der P1-Teilumfang. */
  p0: boolean;
}

const mod = (rest: string): { mac: string; win: string } => ({ mac: `Cmd+${rest}`, win: `Ctrl+${rest}` });
const same = (k: string): { mac: string; win: string } => ({ mac: k, win: k });
const label = (id: string) => "shortcut-" + id.replace(/([a-z0-9])([A-Z])/g, "$1-$2").replace(/\./g, "-").toLowerCase();

function s(id: string, keys: Shortcut["keys"], scope: Scope, p0: boolean, aliases?: Shortcut["aliases"]): Shortcut {
  return { id, keys, scope, labelKey: label(id), p0, ...(aliases ? { aliases } : {}) };
}

/** Verbindliche Standardbelegung (UI-113). */
export const SHORTCUTS: readonly Shortcut[] = [
  s("crawl.start", mod("Enter"), "global", true),
  s("crawl.pause", mod("P"), "global", true),
  s("crawl.new", mod("N"), "global", true),
  s("crawl.open", mod("O"), "global", true),
  s("crawl.saveAs", mod("S"), "global", false),
  s("app.settings", mod(","), "global", true),
  s("config.spider", mod("K"), "global", false),
  s("config.diff", mod("Shift+C"), "global", false),
  s("search.focus", mod("F"), "field", true),
  s("search.clear", same("Esc"), "field", true),
  s("table.copy", mod("C"), "table", true),
  s("table.selectAll", mod("A"), "table", true),
  s("export.view", mod("E"), "global", true),
  s("toptab.next", mod("Alt+Right"), "global", false),
  s("toptab.prev", mod("Alt+Left"), "global", false),
  ...[1, 2, 3, 4, 5, 6, 7, 8, 9].map((n) => s(`toptab.goto.${n}`, mod(String(n)), "global", false)),
  s("filter.open", mod("L"), "global", false),
  s("focus.next", same("F6"), "field", false),
  s("focus.prev", same("Shift+F6"), "field", false),
  s("view.toggleDetail", mod("J"), "global", false),
  s("view.toggleSidebar", mod("Shift+J"), "global", false),
  s("nav.back", { mac: "Cmd+[", win: "Alt+Left" }, "global", false),
  s("nav.forward", { mac: "Cmd+]", win: "Alt+Right" }, "global", false),
  s("browser.open", mod("B"), "global", false),
  s("crawl.recrawl", mod("R"), "global", false),
  s("table.remove", same("Del"), "table", false, { mac: ["Backspace"], win: ["Backspace"] }),
  s("analysis.start", mod("Shift+A"), "global", false),
  s("source.nextDiff", same("F7"), "global", false),
  s("source.prevDiff", same("Shift+F7"), "global", false),
  s("zoom.in", mod("="), "global", false, { mac: ["Cmd++"], win: ["Ctrl++"] }),
  s("zoom.out", mod("-"), "global", false),
  s("zoom.reset", mod("0"), "global", false),
  s("context.menu", same("Shift+F10"), "global", true, { mac: ["ContextMenu"], win: ["ContextMenu"] }),
];

export function shortcutById(id: ActionId): Shortcut | undefined {
  return SHORTCUTS.find((x) => x.id === id);
}

// --- Normalisierung, Anzeige ------------------------------------------------------------------------------------------

const KEY_ALIASES: Record<string, string> = {
  escape: "Esc",
  esc: "Esc",
  enter: "Enter",
  return: "Enter",
  arrowleft: "Left",
  arrowright: "Right",
  arrowup: "Up",
  arrowdown: "Down",
  left: "Left",
  right: "Right",
  up: "Up",
  down: "Down",
  delete: "Del",
  del: "Del",
  backspace: "Backspace",
  contextmenu: "ContextMenu",
  " ": "Space",
  space: "Space",
  tab: "Tab",
  "+": "+",
};

export function normalizeKey(key: string): string {
  const k = KEY_ALIASES[key.toLowerCase()];
  if (k) return k;
  if (/^f\d{1,2}$/i.test(key)) return key.toUpperCase();
  return key.length === 1 ? key.toUpperCase() : key;
}

export interface Combo {
  ctrl: boolean;
  alt: boolean;
  shift: boolean;
  cmd: boolean;
  key: string;
}

/** `Mod+Shift+A` / `Cmd+,` → Teile; `Mod` wird je Plattform aufgelöst. */
export function parseCombo(combo: string, platform: Platform): Combo {
  const parts = combo.split("+");
  // „Cmd++“ / „Ctrl++“: das letzte Element ist die Taste „+“
  let keyPart = parts.pop() ?? "";
  if (keyPart === "" && parts.length > 0) {
    parts.pop();
    keyPart = "+";
  }
  const out: Combo = { ctrl: false, alt: false, shift: false, cmd: false, key: normalizeKey(keyPart) };
  for (const p of parts) {
    switch (p.toLowerCase()) {
      case "mod":
        if (platform === "mac") out.cmd = true;
        else out.ctrl = true;
        break;
      case "ctrl":
        out.ctrl = true;
        break;
      case "alt":
        out.alt = true;
        break;
      case "shift":
        out.shift = true;
        break;
      case "cmd":
        out.cmd = true;
        break;
    }
  }
  return out;
}

export function comboToString(c: Combo): string {
  const mods: string[] = [];
  if (c.ctrl) mods.push("Ctrl");
  if (c.alt) mods.push("Alt");
  if (c.shift) mods.push("Shift");
  if (c.cmd) mods.push("Cmd");
  return [...mods, c.key].join("+");
}

/** Kanonische Form (Modifikatoren in fester Reihenfolge), Grundlage des Vergleichs. */
export function canonicalCombo(combo: string, platform: Platform): string {
  return comboToString(parseCombo(combo, platform));
}

/** Alle Belegungen eines Eintrags (Haupt- und Alias-Kürzel) in kanonischer Form. */
export function combosOf(sc: Shortcut, platform: Platform): string[] {
  const list = [sc.keys[platform], ...(sc.aliases?.[platform] ?? [])];
  return list.map((c) => canonicalCombo(c, platform));
}

const MAC_SYMBOLS: Record<string, string> = {
  Ctrl: "⌃",
  Alt: "⌥",
  Shift: "⇧",
  Cmd: "⌘",
  Enter: "↩",
  Esc: "⎋",
  Left: "←",
  Right: "→",
  Up: "↑",
  Down: "↓",
  Del: "⌦",
  Backspace: "⌫",
  Tab: "⇥",
  Space: "␣",
  ContextMenu: "☰",
};

const WIN_NAMES: Record<string, { de: string; en: string }> = {
  Ctrl: { de: "Strg", en: "Ctrl" },
  Alt: { de: "Alt", en: "Alt" },
  Shift: { de: "Umschalt", en: "Shift" },
  Enter: { de: "Eingabe", en: "Enter" },
  Esc: { de: "Esc", en: "Esc" },
  Left: { de: "←", en: "←" },
  Right: { de: "→", en: "→" },
  Up: { de: "↑", en: "↑" },
  Down: { de: "↓", en: "↓" },
  Del: { de: "Entf", en: "Del" },
  Backspace: { de: "Rücktaste", en: "Backspace" },
  Space: { de: "Leertaste", en: "Space" },
  ContextMenu: { de: "Menütaste", en: "Menu" },
};

/**
 * Plattformgerechte Anzeige: macOS mit Symbolen in Apple-Reihenfolge (⌃⌥⇧⌘), Windows als `Strg+Alt+Umschalt+X` (de)
 * bzw. `Ctrl+Alt+Shift+X` (en).
 */
export function formatShortcut(combo: string, platform: Platform, lang: string): string {
  const c = parseCombo(combo, platform);
  if (platform === "mac") {
    const mods = [c.ctrl && "Ctrl", c.alt && "Alt", c.shift && "Shift", c.cmd && "Cmd"].filter(Boolean) as string[];
    return mods.map((m) => MAC_SYMBOLS[m]).join("") + (MAC_SYMBOLS[c.key] ?? c.key);
  }
  const l = lang === "de" ? "de" : "en";
  const mods = [c.ctrl && "Ctrl", c.alt && "Alt", c.shift && "Shift"].filter(Boolean) as string[];
  const name = (k: string) => WIN_NAMES[k]?.[l] ?? k;
  return [...mods.map(name), name(c.key)].join("+");
}

export function shortcutLabelFor(sc: Shortcut, platform: Platform, lang: string): string {
  return formatShortcut(sc.keys[platform], platform, lang);
}

// --- reservierte Systemkürzel (UI-113, Kollisionsprüfung) --------------------------------------------------------------

export const RESERVED_SHORTCUTS: Record<Platform, readonly string[]> = {
  mac: ["Cmd+H", "Cmd+Alt+H", "Cmd+M", "Cmd+Q", "Cmd+W", "Cmd+Tab", "Cmd+Space", "Cmd+Alt+Esc", "Cmd+Ctrl+F", "Cmd+Shift+3", "Cmd+Shift+4", "Cmd+Shift+5"],
  win: ["Alt+F4", "Alt+Tab", "Ctrl+Esc", "Ctrl+Shift+Esc", "F1"],
};

/** Belegungen, die mit reservierten Systemkürzeln kollidieren (leer = in Ordnung); `Win+…` ist generell gesperrt. */
export function findReservedCollisions(list: readonly Shortcut[] = SHORTCUTS): string[] {
  const out: string[] = [];
  for (const platform of ["mac", "win"] as const) {
    const reserved = new Set(RESERVED_SHORTCUTS[platform].map((c) => canonicalCombo(c, platform)));
    for (const sc of list)
      for (const raw of [sc.keys[platform], ...(sc.aliases?.[platform] ?? [])]) {
        if (/(^|\+)win\+/i.test(raw) || reserved.has(canonicalCombo(raw, platform))) out.push(`${sc.id} (${platform}): ${raw}`);
      }
  }
  return out;
}

/** Doppelte Belegungen innerhalb einer Plattform (derselbe Kürzelstring bei zwei Aktionen mit überschneidendem Scope). */
export function findDuplicateBindings(list: readonly Shortcut[] = SHORTCUTS): string[] {
  const out: string[] = [];
  for (const platform of ["mac", "win"] as const) {
    const seen = new Map<string, Shortcut>();
    for (const sc of list)
      for (const combo of combosOf(sc, platform)) {
        const prev = seen.get(combo);
        if (prev && prev.id !== sc.id) out.push(`${platform}: ${combo} → ${prev.id} und ${sc.id}`);
        else seen.set(combo, sc);
      }
  }
  return out;
}

/** Tabelle für „Hilfe → Tastaturkürzel“ (gleiche Quelle wie Dispatcher und Menü). */
export function shortcutTable(platform: Platform, lang: string): { id: ActionId; labelKey: string; keys: string[]; scope: Scope; p0: boolean }[] {
  return SHORTCUTS.map((sc) => ({
    id: sc.id,
    labelKey: sc.labelKey,
    keys: [sc.keys[platform], ...(sc.aliases?.[platform] ?? [])].map((c) => formatShortcut(c, platform, lang)),
    scope: sc.scope,
    p0: sc.p0,
  }));
}

/** Kürzel, die beim Bearbeiten von Text eine eigene Bedeutung haben – nie als native Menü-Accelerator registrieren (F-5). */
export function isEditingCombo(canonical: string): boolean {
  return /^(Ctrl|Cmd)\+[CVXAZY]$/.test(canonical) || /^(Del|Backspace|Alt\+Left|Alt\+Right|Cmd\+\[|Cmd\+\])$/.test(canonical);
}

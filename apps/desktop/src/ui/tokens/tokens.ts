// Einzige Quelle der Design-Tokens (M2-ui-concept 5.2, UI-120): speist die CSS-Variablen, das Canvas-Theme des
// Grids und den Kontrast-Unit-Test. Werte und Rollen laut Abschnitt 5; neue Tokens nur hier ergänzen.

import { contrast, mix, readableOn, withAlpha } from "./color";

export type ThemeName = "light" | "dark";

/** Voreingestellte Akzentfarbe in `settings.toml` (os-model: `appearance.accent`); bleibt als gespeicherter Default kompatibel; visuell Akzentblau (K2-77). */
export const DEFAULT_ACCENT_SETTING = "#2f855a";
/** Visible preset for the legacy stored default; no settings migration/write is needed. */
export const DEFAULT_ACCENT_DISPLAY = "#3C79CE";

export const TOKEN_NAMES = [
  "bg.app",
  "bg.surface",
  "bg.zebra",
  "bg.selection",
  "bg.selectionInactive",
  "bg.hover",
  "bg.overlay",
  "border",
  "border.strong",
  "text",
  "text.muted",
  "text.disabled",
  "text.onAccent",
  "accent",
  "accent.hover",
  "focus",
  "link",
  "status.2xx",
  "status.3xx",
  "status.4xx",
  "status.5xx",
  "status.none",
  "issue.type.issue",
  "issue.type.warning",
  "issue.type.opportunity",
  "issue.prio.high",
  "issue.prio.medium",
  "issue.prio.low",
  "badge.success",
  "badge.successBg",
  "badge.warning",
  "badge.warningBg",
  "badge.danger",
  "badge.dangerBg",
  "badge.neutral",
  "badge.neutralBg",
  "diff.add",
  "diff.del",
  "diff.addBg",
  "diff.delBg",
] as const;
export type TokenName = (typeof TOKEN_NAMES)[number];
export type TokenMap = Record<TokenName, string>;

const LIGHT: TokenMap = {
  "bg.app": "#EEF0F3",
  "bg.surface": "#FFFFFF",
  "bg.zebra": "#F6F7F9",
  "bg.selection": "#DFEBFF",
  "bg.selectionInactive": "#EAF0FA",
  "bg.hover": "#E6E9ED",
  "bg.overlay": "#FFFFFF",
  border: "#D5D9DF",
  "border.strong": "#737C86",
  text: "#1B1F24",
  "text.muted": "#59626D",
  "text.disabled": "#8B939C",
  "text.onAccent": "#FFFFFF",
  accent: "#0964DC",
  "accent.hover": "#0752B6",
  focus: "#1F62C4",
  link: "#1F5FBF",
  "status.2xx": "#1B1F24",
  "status.3xx": "#A86A00",
  "status.4xx": "#C42B2B",
  "status.5xx": "#C42B2B",
  "status.none": "#7B848E",
  "issue.type.issue": "#C42B2B",
  "issue.type.warning": "#A86A00",
  "issue.type.opportunity": "#1F62C4",
  "issue.prio.high": "#C42B2B",
  "issue.prio.medium": "#B8530F",
  "issue.prio.low": "#666E78",
  "badge.success": "#14652F",
  "badge.successBg": "#DCF5E3",
  "badge.warning": "#744900",
  "badge.warningBg": "#FFF0CC",
  "badge.danger": "#A82027",
  "badge.dangerBg": "#FFE4E5",
  "badge.neutral": "#45515F",
  "badge.neutralBg": "#E9EDF2",
  "diff.add": "#1C7C3A",
  "diff.del": "#C42B2B",
  "diff.addBg": withAlpha("#1C7C3A", 0.12),
  "diff.delBg": withAlpha("#C42B2B", 0.12),
};

const DARK: TokenMap = {
  "bg.app": "#131416",
  "bg.surface": "#1B1D20",
  "bg.zebra": "#212429",
  "bg.selection": "#173859",
  "bg.selectionInactive": "#233241",
  "bg.hover": "#2A2E34",
  "bg.overlay": "#24272C",
  border: "#33373D",
  "border.strong": "#6C747E",
  text: "#E7E9EC",
  "text.muted": "#A3ABB5",
  "text.disabled": "#6C747E",
  "text.onAccent": "#081A30",
  accent: "#58A6FF",
  "accent.hover": "#80BCFF",
  focus: "#6AA8FF",
  link: "#7DB3FF",
  "status.2xx": "#E7E9EC",
  "status.3xx": "#E0A53A",
  "status.4xx": "#F2716B",
  "status.5xx": "#F2716B",
  "status.none": "#8C949E",
  "issue.type.issue": "#F2716B",
  "issue.type.warning": "#E0A53A",
  "issue.type.opportunity": "#6AA8FF",
  "issue.prio.high": "#F2716B",
  "issue.prio.medium": "#F08F4A",
  "issue.prio.low": "#9AA2AC",
  "badge.success": "#A2E8B7",
  "badge.successBg": "#163C2B",
  "badge.warning": "#FFDA8B",
  "badge.warningBg": "#463719",
  "badge.danger": "#FFB1B5",
  "badge.dangerBg": "#4A232A",
  "badge.neutral": "#D0DAE5",
  "badge.neutralBg": "#303943",
  "diff.add": "#56C773",
  "diff.del": "#F2716B",
  "diff.addBg": withAlpha("#56C773", 0.12),
  "diff.delBg": withAlpha("#F2716B", 0.12),
};

export const THEME_TOKENS: Record<ThemeName, Readonly<TokenMap>> = { light: LIGHT, dark: DARK };

/** Acht vorgeprüfte Akzentfarben (≥ 3:1 gegen `bg.app` und `bg.surface` in beiden Themes); die erste ist der Standard. */
export const ACCENT_PALETTE: readonly { id: string; hex: string }[] = [
  { id: "blue", hex: DEFAULT_ACCENT_DISPLAY },
  { id: "green", hex: "#2C8557" },
  { id: "teal", hex: "#218683" },
  { id: "violet", hex: "#915FD7" },
  { id: "pink", hex: "#D04289" },
  { id: "red", hex: "#D14B46" },
  { id: "orange", hex: "#B9612E" },
  { id: "olive", hex: "#708020" },
];

/** Zwölf Segmentfarben (UI-068), je eine Variante für helles und dunkles Theme (≥ 3:1 gegen die Tabellenhintergründe). */
export const SEGMENT_PALETTE: readonly { light: string; dark: string }[] = [
  { light: "#D54F3A", dark: "#E28476" },
  { light: "#A27322", dark: "#CE922C" },
  { light: "#75831C", dark: "#95A723" },
  { light: "#468B1E", dark: "#59B226" },
  { light: "#1E8F2D", dark: "#27B63A" },
  { light: "#1E8C64", dark: "#26B37F" },
  { light: "#208797", dark: "#29ACC1" },
  { light: "#447AD7", dark: "#779EE2" },
  { light: "#7A6BE0", dark: "#9E93E8" },
  { light: "#AA54DB", dark: "#C285E5" },
  { light: "#D433BE", dark: "#E277D4" },
  { light: "#D7447A", dark: "#E47EA3" },
];

/** Label-Textfarbe auf einer Segmentfarbe (UI-020: ≥ 4,5:1). */
export function segmentLabelColor(segmentHex: string): string {
  return readableOn(segmentHex);
}

export interface AccentInfo {
  /** Kontrast der Akzentfarbe gegen `bg.app` und `bg.surface` des Themes (kleinerer Wert). */
  minContrast: number;
  /** Unter 3:1: Warnung im Farbwähler, Markierungen erhalten zusätzlich einen 1-px-Rahmen in Textfarbe. */
  warn: boolean;
}

export function accentInfo(accent: string, theme: ThemeName): AccentInfo {
  const t = THEME_TOKENS[theme];
  const min = Math.min(contrast(accent, t["bg.app"]), contrast(accent, t["bg.surface"]));
  return { minContrast: min, warn: min < 3 };
}

function isDefaultAccent(accent: string | undefined): boolean {
  return accent === undefined || accent === "" || (accent.toLowerCase() === DEFAULT_ACCENT_SETTING || accent.toLowerCase() === DEFAULT_ACCENT_DISPLAY.toLowerCase());
}

/** UI-120/K2-77: show the effective blue preset, retaining the stored legacy alias. */
export function displayAccent(accent: string): string {
  return isDefaultAccent(accent) ? DEFAULT_ACCENT_DISPLAY : accent;
}

/** Selektionsfarbe aus der Akzentfarbe: Anteil so wählen, dass `text` ≥ 4,5:1 bleibt. */
function deriveSelection(surface: string, accent: string, text: string, start: number): string {
  let t = start;
  for (;;) {
    const c = mix(surface, accent, t);
    if (contrast(text, c) >= 4.5 || t <= 0.02) return c;
    t *= 0.8;
  }
}

/** Alle Tokens eines Themes; eine frei gewählte Akzentfarbe ersetzt Akzent und die davon abgeleiteten Werte. */
export function resolveTokens(theme: ThemeName, accent?: string): TokenMap {
  const base = THEME_TOKENS[theme];
  if (isDefaultAccent(accent)) return { ...base };
  const a = accent as string;
  const dark = theme === "dark";
  const text = base.text;
  return {
    ...base,
    accent: a.toUpperCase(),
    "accent.hover": mix(a, dark ? "#FFFFFF" : "#000000", 0.18),
    "text.onAccent": readableOn(a),
    "bg.selection": deriveSelection(base["bg.surface"], a, text, dark ? 0.34 : 0.22),
    "bg.selectionInactive": deriveSelection(base["bg.surface"], a, text, dark ? 0.16 : 0.1),
  };
}

// --- Typografie, Größen (5.1, 5.3) ----------------------------------------------------------------------------------

export type Platform = "mac" | "win";

/** Schriftstapel ohne Produktnamen der Systemschrift (5.1). */
export const FONT_STACKS: Record<Platform, { ui: string; mono: string }> = {
  mac: {
    ui: "system-ui, -apple-system, sans-serif",
    mono: "ui-monospace, Menlo, monospace",
  },
  win: {
    ui: '"Segoe UI Variable Text", "Segoe UI", system-ui, sans-serif',
    mono: 'Consolas, "Cascadia Mono", monospace',
  },
};

export type Density = "compact" | "normal";

export const SIZES = {
  space: [2, 4, 6, 8, 12, 16, 24, 32] as const,
  rowHeight: { compact: 28, normal: 44 } as const,
  headerHeight: { compact: 32, normal: 44 } as const,
  cellPaddingX: 6,
  toolbarHeight: 32,
  toolbarControl: 24,
  control: 28,
  tab: 28,
  statusbar: 24,
  splitter: 4,
  splitterGrip: 8,
  radiusControl: 4,
  radiusDialog: 8,
  fontUi: 13,
  lineUi: 18,
  fontTable: { compact: 13, normal: 14 } as const,
  fontSmall: 11,
  lineSmall: 14,
  fontMono: 12,
  lineMono: 17,
  fontDialogTitle: 17,
} as const;

/** Zoomstufen (UI-120). */
export const ZOOM_STEPS = [80, 90, 100, 110, 125, 150] as const;

export function cssVarName(token: string): string {
  return "--os-" + token.replace(/\./g, "-").replace(/([a-z0-9])([A-Z])/g, "$1-$2").toLowerCase();
}

/** CSS-Variablen eines Themes (für `:root`). */
export function tokensToCssVars(tokens: TokenMap): Record<string, string> {
  const out: Record<string, string> = {};
  for (const name of TOKEN_NAMES) out[cssVarName(name)] = tokens[name];
  return out;
}

// --- Canvas-Theme (Grid, M2-S01 bildet es auf das Glide-Theme ab) -----------------------------------------------------

export interface CanvasTheme {
  bgCell: string;
  bgCellZebra: string;
  bgHeader: string;
  bgSelection: string;
  bgSelectionInactive: string;
  textCell: string;
  textMuted: string;
  textHeader: string;
  accent: string;
  focus: string;
  border: string;
  /** Markierungen erhalten einen 1-px-Rahmen in Textfarbe (Akzent < 3:1). */
  selectionOutline: string | null;
  fontFamily: string;
  monoFamily: string;
  fontSizePx: number;
  rowHeight: number;
  headerHeight: number;
  cellPaddingX: number;
  zebra: boolean;
  status: Record<"2xx" | "3xx" | "4xx" | "5xx" | "none", string>;
}

export function canvasTheme(
  tokens: TokenMap,
  opts: { platform: Platform; density: Density; zebra: boolean; accentWarn?: boolean },
): CanvasTheme {
  const fonts = FONT_STACKS[opts.platform];
  return {
    bgCell: tokens["bg.surface"],
    bgCellZebra: opts.zebra ? tokens["bg.zebra"] : tokens["bg.surface"],
    bgHeader: tokens["bg.app"],
    bgSelection: tokens["bg.selection"],
    bgSelectionInactive: tokens["bg.selectionInactive"],
    textCell: tokens.text,
    textMuted: tokens["text.muted"],
    textHeader: tokens.text,
    accent: tokens.accent,
    focus: tokens.focus,
    border: tokens.border,
    selectionOutline: opts.accentWarn ? tokens.text : null,
    fontFamily: fonts.ui,
    monoFamily: fonts.mono,
    fontSizePx: SIZES.fontTable[opts.density],
    rowHeight: SIZES.rowHeight[opts.density],
    headerHeight: SIZES.headerHeight[opts.density],
    cellPaddingX: SIZES.cellPaddingX,
    zebra: opts.zebra,
    status: {
      "2xx": tokens["status.2xx"],
      "3xx": tokens["status.3xx"],
      "4xx": tokens["status.4xx"],
      "5xx": tokens["status.5xx"],
      none: tokens["status.none"],
    },
  };
}

/** Canvas-Theme aus Systemfarben (Windows-Kontrastdesigns, `forced-colors`): das Canvas folgt dem Modus nicht selbst. */
export function forcedColorsCanvasTheme(
  system: { canvas: string; canvasText: string; highlight: string; highlightText: string; grayText: string },
  opts: { platform: Platform; density: Density },
): CanvasTheme {
  const fonts = FONT_STACKS[opts.platform];
  return {
    bgCell: system.canvas,
    bgCellZebra: system.canvas,
    bgHeader: system.canvas,
    bgSelection: system.highlight,
    bgSelectionInactive: system.highlight,
    textCell: system.canvasText,
    textMuted: system.grayText,
    textHeader: system.canvasText,
    accent: system.highlight,
    focus: system.highlight,
    border: system.canvasText,
    selectionOutline: system.canvasText,
    fontFamily: fonts.ui,
    monoFamily: fonts.mono,
    fontSizePx: SIZES.fontTable[opts.density],
    rowHeight: SIZES.rowHeight[opts.density],
    headerHeight: SIZES.headerHeight[opts.density],
    cellPaddingX: SIZES.cellPaddingX,
    zebra: false,
    status: {
      "2xx": system.canvasText,
      "3xx": system.canvasText,
      "4xx": system.canvasText,
      "5xx": system.canvasText,
      none: system.grayText,
    },
  };
}

/** K2-77 Canvas decorations share the design-system spacing/radii. */
export const GRID_DECORATION = {
  badgeHeight: 20, badgeRadius: 8, badgePadding: 8, badgeInset: 4,
  checkboxSize: 12, checkboxInset: 8, checkboxRadius: 4,
  addressPadding: 28, addressAction: 24, addressReserved: 52,
  labelInset: 8,
} as const;

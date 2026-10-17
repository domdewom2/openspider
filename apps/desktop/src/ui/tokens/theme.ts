// Anwenden von Theme, Akzent, Dichte, Zebra und Zoom auf ein Dokument (UI-120).
import {
  accentInfo,
  cssVarName,
  resolveTokens,
  SIZES,
  tokensToCssVars,
  type Density,
  type Platform,
  type ThemeName,
} from "./tokens";
import { FONT_STACKS } from "./tokens";

export type ThemeSetting = "light" | "dark" | "system";

export interface AppearanceState {
  theme: ThemeSetting;
  accent: string;
  zoomPct: number;
  density: Density;
  zebra: boolean;
}

export const DEFAULT_APPEARANCE: AppearanceState = {
  theme: "system",
  accent: "#2f855a",
  zoomPct: 100,
  density: "compact",
  zebra: true,
};

/** Löst „System“ gegen die Systemeinstellung auf. */
export function resolveTheme(setting: ThemeSetting, systemPrefersDark: boolean): ThemeName {
  if (setting === "system") return systemPrefersDark ? "dark" : "light";
  return setting;
}

export function systemPrefersDark(win: Pick<Window, "matchMedia"> = window): boolean {
  try {
    return win.matchMedia("(prefers-color-scheme: dark)").matches;
  } catch {
    return false;
  }
}

/** Schreibt alle Tokens und Schalter auf das Wurzelelement; liefert das aufgelöste Theme und die Akzentwarnung. */
export function applyAppearance(
  root: HTMLElement,
  state: AppearanceState,
  env: { platform: Platform; systemDark: boolean },
): { theme: ThemeName; accentWarn: boolean; accentContrast: number } {
  const theme = resolveTheme(state.theme, env.systemDark);
  const tokens = resolveTokens(theme, state.accent);
  const vars = tokensToCssVars(tokens);
  for (const [k, v] of Object.entries(vars)) root.style.setProperty(k, v);
  const info = accentInfo(tokens.accent, theme);
  const fonts = FONT_STACKS[env.platform];
  root.style.setProperty("--os-font-ui", fonts.ui);
  root.style.setProperty("--os-font-mono", fonts.mono);
  root.style.setProperty("--os-row-height", `${SIZES.rowHeight[state.density]}px`);
  root.style.setProperty("--os-header-height", `${SIZES.headerHeight[state.density]}px`);
  root.style.setProperty("--os-font-table", `${SIZES.fontTable[state.density]}px`);
  root.style.setProperty("--os-zoom", String(state.zoomPct / 100));
  root.dataset.theme = theme;
  root.dataset.themeSetting = state.theme;
  root.dataset.density = state.density;
  root.dataset.zebra = state.zebra ? "on" : "off";
  root.dataset.accentWarn = info.warn ? "on" : "off";
  root.dataset.platform = env.platform;
  root.style.colorScheme = theme;
  return { theme, accentWarn: info.warn, accentContrast: info.minContrast };
}

/** Variablenname eines Tokens, z. B. `varOf("bg.app")` → `var(--os-bg-app)`. */
export function varOf(token: string): string {
  return `var(${cssVarName(token)})`;
}

/** Liest die Systemfarben für das Canvas-Theme im Kontrastmodus (`forced-colors`). */
export function readSystemColors(doc: Document = document): {
  canvas: string;
  canvasText: string;
  highlight: string;
  highlightText: string;
  grayText: string;
} {
  const probe = doc.createElement("span");
  doc.body.appendChild(probe);
  const read = (value: string) => {
    probe.style.color = value;
    return getComputedStyle(probe).color;
  };
  const out = {
    canvas: read("Canvas"),
    canvasText: read("CanvasText"),
    highlight: read("Highlight"),
    highlightText: read("HighlightText"),
    grayText: read("GrayText"),
  };
  probe.remove();
  return out;
}

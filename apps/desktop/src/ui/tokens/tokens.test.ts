import { describe, expect, it } from "vitest";
import { contrast, isHexColor, mix, readableOn } from "./color";
import {
  accentInfo,
  ACCENT_PALETTE,
  canvasTheme,
  resolveTokens,
  SEGMENT_PALETTE,
  segmentLabelColor,
  SIZES,
  THEME_TOKENS,
  TOKEN_NAMES,
  type ThemeName,
  type TokenName,
} from "./tokens";
import { applyAppearance, DEFAULT_APPEARANCE, resolveTheme } from "./theme";

const THEMES: ThemeName[] = ["light", "dark"];

/** Paare (Vordergrund, Hintergründe, Mindestkontrast) laut UI-120 und M2-ui-concept 5.2. */
const TEXT_ON_SURFACES: TokenName[] = ["bg.surface", "bg.zebra", "bg.selection", "bg.app"];
const PAIRS: { fg: TokenName; bgs: TokenName[]; min: number; why: string }[] = [
  { fg: "text", bgs: TEXT_ON_SURFACES, min: 4.5, why: "Fließtext" },
  { fg: "text.muted", bgs: ["bg.surface", "bg.zebra", "bg.app"], min: 4.5, why: "Nebentext (in Auswahl gilt text)" },
  { fg: "link", bgs: ["bg.surface", "bg.zebra", "bg.app"], min: 4.5, why: "Verweise" },
  { fg: "text.onAccent", bgs: ["accent"], min: 4.5, why: "Text auf Akzentfläche" },
  { fg: "accent", bgs: ["bg.surface", "bg.zebra", "bg.app", "bg.selection"], min: 3, why: "Akzent als Symbol" },
  { fg: "focus", bgs: ["bg.surface", "bg.zebra", "bg.app", "bg.selection"], min: 3, why: "Fokusring" },
  { fg: "border.strong", bgs: ["bg.surface", "bg.app"], min: 3, why: "Rahmen von Eingabefeldern" },
  { fg: "status.3xx", bgs: ["bg.surface", "bg.zebra", "bg.selection"], min: 3, why: "Statusmarke 3xx" },
  { fg: "status.4xx", bgs: ["bg.surface", "bg.zebra", "bg.selection"], min: 3, why: "Statusmarke 4xx" },
  { fg: "status.5xx", bgs: ["bg.surface", "bg.zebra", "bg.selection"], min: 3, why: "Statusmarke 5xx" },
  { fg: "status.none", bgs: ["bg.surface", "bg.zebra", "bg.selection"], min: 3, why: "Statusmarke ohne Antwort" },
  { fg: "status.2xx", bgs: ["bg.surface", "bg.zebra", "bg.selection"], min: 4.5, why: "Statuszahl" },
  { fg: "issue.type.issue", bgs: ["bg.surface", "bg.zebra", "bg.selection"], min: 3, why: "Issue-Symbol" },
  { fg: "issue.type.warning", bgs: ["bg.surface", "bg.zebra", "bg.selection"], min: 3, why: "Warnung-Symbol" },
  { fg: "issue.type.opportunity", bgs: ["bg.surface", "bg.zebra", "bg.selection"], min: 3, why: "Opportunity-Symbol" },
  { fg: "issue.prio.high", bgs: ["bg.surface", "bg.zebra"], min: 4.5, why: "Prioritätstext" },
  { fg: "issue.prio.medium", bgs: ["bg.surface", "bg.zebra"], min: 4.5, why: "Prioritätstext" },
  { fg: "issue.prio.low", bgs: ["bg.surface", "bg.zebra"], min: 4.5, why: "Prioritätstext" },
  { fg: "diff.add", bgs: ["bg.surface", "bg.zebra"], min: 3, why: "Diff-Randmarke" },
  { fg: "diff.del", bgs: ["bg.surface", "bg.zebra"], min: 3, why: "Diff-Randmarke" },
];

describe("Design-Tokens: Kontrast (UI-120)", () => {
  for (const theme of THEMES) {
    // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
    it(`alle Token-Paare erreichen die Mindestkontraste im Theme ${theme}`, () => {
      const t = THEME_TOKENS[theme];
      const failures: string[] = [];
      for (const { fg, bgs, min, why } of PAIRS) {
        for (const bg of bgs) {
          const c = contrast(t[fg], t[bg]);
          if (c < min) failures.push(`${theme}: ${fg} auf ${bg} = ${c.toFixed(2)} < ${min} (${why})`);
        }
      }
      expect(failures).toEqual([]);
    });
  }

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("Status-, Issue-, Diff- und Segmentfarben haben je eine Hell- und eine Dunkel-Variante mit >= 3:1", () => {
    for (const name of TOKEN_NAMES.filter((n) => /^(status|issue\.type|diff\.(add|del)$)/.test(n))) {
      expect(THEME_TOKENS.light[name], name).toBeTruthy();
      expect(THEME_TOKENS.dark[name], name).toBeTruthy();
    }
    expect(SEGMENT_PALETTE).toHaveLength(12);
    for (const seg of SEGMENT_PALETTE) {
      for (const theme of THEMES) {
        const hex = theme === "light" ? seg.light : seg.dark;
        for (const bg of ["bg.surface", "bg.zebra"] as const) {
          expect(contrast(hex, THEME_TOKENS[theme][bg])).toBeGreaterThanOrEqual(3);
        }
      }
      // Segment-Label (UI-020): automatisch Schwarz oder Weiß mit >= 4,5:1
      for (const hex of [seg.light, seg.dark]) {
        expect(contrast(hex, segmentLabelColor(hex))).toBeGreaterThanOrEqual(4.5);
      }
    }
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("alle acht Akzentfarben der Palette haben >= 3:1 gegen bg.app und bg.surface in beiden Themes", () => {
    expect(ACCENT_PALETTE).toHaveLength(8);
    for (const { hex } of ACCENT_PALETTE) {
      for (const theme of THEMES) {
        const info = accentInfo(hex, theme);
        expect(info.minContrast, `${hex} ${theme}`).toBeGreaterThanOrEqual(3);
        expect(info.warn).toBe(false);
      }
    }
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("eine Akzentfarbe unter 3:1 (#FFFF66) löst die Warnung mit Kontrastwert aus", () => {
    const light = accentInfo("#FFFF66", "light");
    expect(light.warn).toBe(true);
    expect(light.minContrast).toBeLessThan(3);
    expect(accentInfo("#FFFF66", "dark").warn).toBe(false);
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("abgeleitete Auswahlfarben halten text >= 4,5:1 auch bei freier Akzentfarbe", () => {
    for (const accent of ["#FFFF66", "#FF0000", "#0000FF", "#222222", "#00FFAA"]) {
      for (const theme of THEMES) {
        const t = resolveTokens(theme, accent);
        expect(contrast(t.text, t["bg.selection"]), `${accent} ${theme}`).toBeGreaterThanOrEqual(4.5);
        expect(contrast(t["text.onAccent"], t.accent)).toBeGreaterThanOrEqual(4.5);
      }
    }
  });
});

describe("Farbrechnung", () => {
  it("Kontrast Schwarz/Weiß ist 21", () => {
    expect(contrast("#000000", "#FFFFFF")).toBeCloseTo(21, 5);
  });
  it("mix(a, b, 0) = a und mix(a, b, 1) = b", () => {
    expect(mix("#102030", "#FFEEDD", 0)).toBe("#102030");
    expect(mix("#102030", "#FFEEDD", 1)).toBe("#FFEEDD");
  });
  it("readableOn wählt die lesbarere Farbe", () => {
    expect(readableOn("#FFFFFF")).toBe("#000000");
    expect(readableOn("#000000")).toBe("#FFFFFF");
  });
  it("isHexColor erwartet #rrggbb", () => {
    expect(isHexColor("#2f855a")).toBe(true);
    expect(isHexColor("2f855a")).toBe(false);
    expect(isHexColor("#fff")).toBe(false);
  });
});

describe("Theme anwenden", () => {
  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("System folgt der Systemeinstellung, Hell/Dunkel sind fest", () => {
    expect(resolveTheme("system", true)).toBe("dark");
    expect(resolveTheme("system", false)).toBe("light");
    expect(resolveTheme("light", true)).toBe("light");
    expect(resolveTheme("dark", false)).toBe("dark");
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("applyAppearance schreibt Tokens, Dichte, Zebra und Zoom auf das Wurzelelement", () => {
    const root = document.createElement("div");
    const r = applyAppearance(root, { ...DEFAULT_APPEARANCE, theme: "dark", zoomPct: 125, density: "normal" }, { platform: "mac", systemDark: false });
    expect(r.theme).toBe("dark");
    expect(root.dataset.theme).toBe("dark");
    expect(root.dataset.density).toBe("normal");
    expect(root.style.getPropertyValue("--os-bg-surface").toUpperCase()).toBe("#1B1D20");
    expect(root.style.getPropertyValue("--os-row-height")).toBe("44px");
    expect(root.style.getPropertyValue("--os-zoom")).toBe("1.25");
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("Standard ist System, 28 px kompakt und Zebra an; Zeilenhöhe normal bei 150 % = 66 px", () => {
    expect(DEFAULT_APPEARANCE).toMatchObject({ theme: "system", density: "compact", zebra: true, zoomPct: 100 });
    expect(SIZES.rowHeight.compact).toBe(28);
    expect(SIZES.rowHeight.normal * 1.5).toBe(66);
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("Akzentwarnung setzt im Canvas-Theme einen Rahmen in Textfarbe", () => {
    const tokens = resolveTokens("light", "#FFFF66");
    const warn = canvasTheme(tokens, { platform: "win", density: "compact", zebra: true, accentWarn: true });
    const ok = canvasTheme(resolveTokens("light"), { platform: "win", density: "compact", zebra: false });
    expect(warn.selectionOutline).toBe(tokens.text);
    expect(ok.selectionOutline).toBeNull();
    expect(ok.bgCellZebra).toBe(ok.bgCell);
    expect(ok.rowHeight).toBe(28);
  });
});

// spec: UI-126
it("K2-77 badge labels reach 4.5:1 in both themes", () => {
  for (const theme of THEMES) for (const kind of ["success", "warning", "danger", "neutral"] as const) {
    expect(contrast(THEME_TOKENS[theme][`badge.${kind}`], THEME_TOKENS[theme][`badge.${kind}Bg`])).toBeGreaterThanOrEqual(4.5);
  }
});

// Statische Prüfungen (M2-ui-concept 7.1, `npm run check:static`): schlagen den Build fehlschlagen lassen, wenn Regeln des
// Frontends verletzt werden. Quelltexte werden per import.meta.glob eingelesen (kein Dateisystemzugriff nötig).
import { describe, expect, it } from "vitest";
import pkg from "../../../package.json";
import { checkCatalog } from "../../i18n/check";
import { i18n, loadFtlFiles } from "../../i18n";
import { SHORTCUTS, shortcutTable, findDuplicateBindings, findReservedCollisions } from "../shortcuts";
import { EMPTY_CODES, emptyMessageKey } from "../emptyState/codes";
import { MENU_ORDER } from "../menu/types";
import { menuLabelKey } from "../menu/build";
import { actionLabelKey } from "../primitives/EmptyState";
import { findMissingRegisterEntries } from "../emptyState/register";
import { ACCENT_PALETTE } from "../tokens/tokens";
import filtersM1 from "../../../../../docs/plan/M1-filters.toml?raw";
import filtersM2 from "../../../../../docs/plan/M2-filters.toml?raw";

const sources = import.meta.glob<string>(["/src/**/*.{ts,tsx,css}", "!/src/**/*.test.{ts,tsx}", "!/src/ui/static/**", "!/src/ipc/gen/**"], {
  query: "?raw",
  import: "default",
  eager: true,
});

const pkgAny = pkg as { dependencies: Record<string, string>; devDependencies: Record<string, string> };

describe("i18n: Vollständigkeit (UI-121, CI)", () => {
  // spec: UI-121
  it("alle Schlüssel sind in de und en vorhanden, gleiche Platzhalter, eindeutig, Verweise auflösbar", () => {
    const report = checkCatalog(loadFtlFiles());
    expect(report.problems).toEqual([]);
    expect(report.keys).toBeGreaterThan(100);
    expect(report.completeness.de).toBe(100);
    expect(report.completeness.en).toBe(100);
  });

  // spec: UI-121
  it("kein Fluent-Fehler beim Laden der Bundles", () => {
    expect(i18n.errors()).toEqual([]);
    expect(i18n.languages().slice(0, 3)).toEqual(["en", "de"]);
  });

  // spec: UI-121
  it("Menütitel, Kürzelbeschriftungen, Leerzustände und Farbnamen haben Übersetzungen in beiden Sprachen", () => {
    const keys = [
      ...MENU_ORDER.mac.map(menuLabelKey),
      ...SHORTCUTS.map((s) => s.labelKey),
      ...EMPTY_CODES.map(emptyMessageKey),
      ...["openSetting", "startAnalysis", "clearSearch", "connectApi"].map((kind) => actionLabelKey({ kind } as never)),
      ...ACCENT_PALETTE.map((c) => `color-${c.id}`),
    ];
    const missing = keys.flatMap((k) => ["de", "en"].filter((l) => !i18n.has(l, k)).map((l) => `${l}:${k}`));
    expect(missing).toEqual([]);
  });
});

describe("Kürzelregister (UI-113, CI)", () => {
  // spec: UI-113
  it("Hilfe-Tabelle, Menüanzeige und Register enthalten dieselben Einträge; keine Kollision mit reservierten Kürzeln", () => {
    for (const platform of ["mac", "win"] as const) {
      const rows = shortcutTable(platform, "en");
      expect(rows.map((r) => r.id)).toEqual(SHORTCUTS.map((s) => s.id));
    }
    expect(findReservedCollisions()).toEqual([]);
    expect(findDuplicateBindings()).toEqual([]);
  });
});

describe("Voraussetzungsregister (UI-016, CI)", () => {
  /** Alle Filter aus M1-filters.toml und M2-filters.toml als `Tab:Filter`. */
  function requiredFilters(text: string): { kind: "filter"; id: string }[] {
    const out: { kind: "filter"; id: string }[] = [];
    for (const block of text.split(/^\[\[tab\]\]/m).slice(1)) {
      const tab = /^id\s*=\s*"([^"]+)"/m.exec(block)?.[1];
      if (!tab) continue;
      for (const m of block.matchAll(/\{\s*id\s*=\s*"([^"]+)"/g)) out.push({ kind: "filter", id: `${tab}:${m[1]}` });
    }
    return out;
  }

  // spec: UI-016
  it("liest alle Filter aus den beiden Kennungsdateien", () => {
    const all = [...requiredFilters(filtersM1), ...requiredFilters(filtersM2)];
    expect(all.length).toBeGreaterThan(100);
    expect(all.some((f) => f.id === "Internal:All")).toBe(true);
    expect(new Set(all.map((f) => f.id)).size).toBeGreaterThan(100);
  });

  // spec: UI-016
  it("jeder registrierte Filter hat einen Eintrag im Register des Kerns (core/ui/prerequisites.toml), sobald es existiert", async () => {
    const registers = import.meta.glob("../../../../../core/ui/prerequisites.toml", { query: "?raw", import: "default", eager: true }) as Record<string, string>;
    const text = Object.values(registers)[0];
    const all = [...requiredFilters(filtersM1), ...requiredFilters(filtersM2)];
    if (text === undefined) {
      // Das Register liefert der Kern (noch kein Besitzer im Plan): die Auswertung ist getestet (emptyState.test.ts),
      // die Prüfung gegen die Datei läuft, sobald sie im Repo liegt.
      console.warn("core/ui/prerequisites.toml fehlt – Abdeckungsprüfung des Voraussetzungsregisters wartet auf den Kern");
      return;
    }
    const entries = [...text.matchAll(/^\s*id\s*=\s*"([^"]+)"/gm)].map((m) => ({ kind: "filter" as const, id: m[1] as string, conditions: [] }));
    expect(findMissingRegisterEntries(entries, all)).toEqual([]);
  });
});

describe("Quelltext-Regeln (CI)", () => {
  const files = Object.entries(sources);

  it("liest die Quelltexte ein", () => {
    expect(files.length).toBeGreaterThan(50);
  });

  // spec: UI-120
  it("Glide ist exakt gepinnt (kein ^ oder ~)", () => {
    expect(pkgAny.dependencies["@glideapps/glide-data-grid"]).toBe("6.0.4-alpha24");
  });

  it("alle in M2-F04 ergänzten Abhängigkeiten sind exakt gepinnt", () => {
    const legacy = new Set([
      "@tauri-apps/api",
      "@tauri-apps/cli",
      "react",
      "react-dom",
      "@testing-library/dom",
      "@testing-library/jest-dom",
      "@testing-library/react",
      "@types/react",
      "@types/react-dom",
      "@vitejs/plugin-react",
      "jsdom",
      "typescript",
      "vite",
      "vitest",
    ]);
    const loose = Object.entries({ ...pkgAny.dependencies, ...pkgAny.devDependencies })
      .filter(([name, range]) => !legacy.has(name) && !/^\d/.test(range))
      .map(([name, range]) => `${name}@${range}`);
    expect(loose).toEqual([]);
  });

  // spec: UI-120
  it("alle Pflicht-Bibliotheken des Fundaments sind eingetragen", () => {
    const all = { ...pkgAny.dependencies, ...pkgAny.devDependencies };
    for (const name of [
      "react-aria-components",
      "zustand",
      "@tanstack/react-query",
      "zod",
      "@fluent/bundle",
      "@fluent/langneg",
      "lucide-react",
      "@codemirror/state",
      "@codemirror/view",
      "@playwright/test",
      "@axe-core/playwright",
      "vitest-canvas-mock",
    ])
      expect(all[name], name).toBeDefined();
  });

  it("kein measureText in der Oberfläche: Pixelbreiten liefert der Kern (UI-038)", () => {
    const offenders = files.filter(([, text]) => /\bmeasureText\s*\(/.test(text)).map(([path]) => path);
    expect(offenders).toEqual([]);
  });

  it("@tauri-apps/api/core wird nur im Tauri-Transport benutzt (alle Commands laufen über den IPC-Client)", () => {
    const offenders = files.filter(([path, text]) => /@tauri-apps\/api\/core/.test(text) && !path.endsWith("/ipc/transport/tauri.ts")).map(([p]) => p);
    expect(offenders).toEqual([]);
  });

  it("keine Datenbank- oder Dateizugriffe im Frontend (rusqlite, sqlite, node:fs)", () => {
    const offenders = files.filter(([, text]) => /rusqlite|from "node:fs"|better-sqlite|sql\.js/.test(text)).map(([p]) => p);
    expect(offenders).toEqual([]);
  });

  it("Schriftstapel nennen keine Produktnamen der macOS-Systemschrift (system-ui, -apple-system, ui-monospace, Menlo)", () => {
    const offenders = files.filter(([, text]) => /BlinkMacSystemFont|San Fran/.test(text)).map(([p]) => p);
    expect(offenders).toEqual([]);
  });
});

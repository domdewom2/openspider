import { readdirSync, readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { i18n } from "./index";
import { exportDisplayName } from "./exportNames";

// The shipped core catalog is the fixture: additions there must also have UI display names.
function namesInCore(): string[] {
  const names = new Set<string>();
  for (const dir of ["../../crates/os-export/src/catalog", "../../crates/os-export/src/reports"]) {
    for (const name of readdirSync(dir).filter(n => n.endsWith(".rs"))) {
      const source = readFileSync(`${dir}/${name}`, "utf8").split("#[cfg(test)]")[0]!;
      for (const match of source.matchAll(/(?:name:\s*|(?:NAME|BULK_NAME): &str = )"([^"{}]+)"/g)) names.add(match[1]!);
    }
  }
  return [...names].filter(n => n !== "issues");
}

// spec: UI-121
// spec: OUT-271
it("translates the shipped export and report catalog through the same resolver for menus and dialogs", () => {
  const missing = namesInCore().filter(name => !i18n.has("de", name) || !i18n.has("en", name));
  expect(missing).toEqual([]);
  expect(i18n.translate("de", "Links:All Inlinks")).toBe("Links: Alle eingehenden Links");
  expect(i18n.translate("de", "Crawl Overview")).toBe("Crawl-Übersicht");
  expect(i18n.translate("de", "Accessibility:WCAG 2.0 AA")).toBe("Barrierefreiheit: WCAG 2.0 AA");
  expect(i18n.translate("en", "Accessibility:WCAG 2.0 AA")).toBe("Accessibility:WCAG 2.0 AA");
  expect(i18n.translate("de", "detail-tab-serp")).toBe("SERP-Snippet");
  expect(i18n.translate("en", "detail-tab-serp")).toBe("SERP-Snippet");
});

// spec: UI-121
it("leaves unknown names to the caller's canonical fallback without partially translated names", () => {
  expect(exportDisplayName("Future:Unknown", () => null)).toBeNull();
  expect(i18n.translate("de", "future-key", undefined, { fallback: "Future" })).toBe("Future");
});

// spec: UI-121
it("translates numeric image limits and custom exports while preserving user names as data", () => {
  expect(i18n.translate("de", "Images:Images Over 100 KB Inlinks")).toBe("Bilder: Bilder über 100 KB – eingehende Links");
  expect(i18n.translate("de", "Custom Search:Stock: EU Inlinks")).toBe("Eigene Suche: Stock: EU – eingehende Links");
  expect(i18n.translate("de", "Custom Extraction:Extractor 2 Inlinks")).toBe("Eigene Extraktion: Extraktor 2 – eingehende Links");
});

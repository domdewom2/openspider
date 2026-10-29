import { readdirSync, readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { createI18n, i18n } from "./index";
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
  expect(i18n.translate("de", "Custom Search:Stock: EU Inlinks")).toBe("Eigene Suche: Stock: EU Inlinks");
  expect(i18n.translate("de", "Custom Extraction:Extractor 2 Inlinks")).toBe("Eigene Extraktion: Extractor 2 Inlinks");
});

// spec: UI-121
it("translates core prerequisite diagnostics in UI menus without changing CLI/report data", () => {
  const reason = "report 'Crawl Overview' requires the crawl analysis: 2 filter(s) stay empty until it has run (Canonicals:Unlinked, Pagination:Unlinked Pagination URLs); run the crawl analysis first";
  expect(i18n.translate("de", reason)).toBe("Crawl-Übersicht: Zuerst die Crawl-Analyse ausführen.");
  expect(i18n.translate("en", reason)).toBe("Crawl Overview: Run the crawl analysis first.");
  expect(i18n.translate("de", "report 'Orphan Pages' requires SitemapProcessed; report 'Orphan Pages' requires OrphanSourceAvailable")).toContain("Sitemap");
  expect(i18n.translate("de", "report 'Cookie Summary' requires cookie extraction")).toBe("Cookie-Zusammenfassung: Erfasste Cookies sind erforderlich.");
  expect(i18n.translate("de", "report 'Crawl Overview' requires future capability")).toBe("Crawl-Übersicht: Eine Voraussetzung für diesen Export fehlt.");
});

// spec: UI-121
it("uses catalog metadata to distinguish user names from generated Inlinks and default names", () => {
  const label = (group: string, name: string, inlinks: boolean, generatedIndex?: number) =>
    i18n.translate("de", `custom-export:${JSON.stringify({ group, name, inlinks, generatedIndex })}`);
  for (const [group, title] of [["search", "Eigene Suche"], ["extraction", "Eigene Extraktion"]]) {
    expect(label(group!, "Screenshots", false)).toBe(`${title}: Screenshots`);
    expect(label(group!, "Stock Inlinks", false)).toBe(`${title}: Stock Inlinks`);
    expect(label(group!, "Stock", true)).toBe(`${title}: Stock – eingehende Links`);
    expect(label(group!, "Stock Inlinks", true)).toBe(`${title}: Stock Inlinks – eingehende Links`);
    expect(label(group!, "Extractor 2", false)).toBe(`${title}: Extractor 2`);
    expect(label(group!, "Stock: EU", false)).toBe(`${title}: Stock: EU`);
  }
  expect(label("extraction", "Extractor 2", true, 2)).toBe("Eigene Extraktion: Extraktor 2 – eingehende Links");
  expect(label("search", "Filter 1", false, 1)).toBe("Eigene Suche: Filter 1");
  expect(i18n.translate("de", "Custom Search:Screenshots")).toBe("Eigene Suche: Screenshots");
  expect(i18n.translate("de", "Custom Search:Stock Inlinks")).toBe("Eigene Suche: Stock Inlinks");
});

// spec: UI-121 [P1]
it("falls back to the English catalog label when a community resource lacks a generated export template", () => {
  const source = (lang: string, source: string) => ({ lang, source, path: `${lang}.ftl`, feature: "fixture" });
  const catalog = createI18n([
    source("en", "tab-custom-search = Custom Search\nexport-name-inlinks = { $name } Inlinks"),
    source("pl", "tab-custom-search = Wyszukiwanie"),
  ]);
  const key = `custom-export:${JSON.stringify({ group: "search", name: "Stock", inlinks: true })}`;
  expect(catalog.translate("pl", key)).toBe("Custom Search:Stock Inlinks");
});

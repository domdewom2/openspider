// Vertragsdaten der Fach-Detailtabs (M2-U21), abgeleitet aus den Fixture-Beschreibungen in docs/spec/05-ui.md
// (fx-ui-serp, fx-ui-js, fx-ui-dup, fx-ui-sd, fx-ui-spell, fx-ui-a11y); die gemeinsamen Spec-Fixtures gehören den Kernpaketen.
import type { SdNode } from "../../../../ipc/gen/SdNode";
import type { SerpData } from "../../../../ipc/gen/SerpData";

export const CRAWL_ID = "fixture";
export const SERP: Record<number, SerpData & { address: string }> = {
  1: { address: "https://site.test/", title: "Start", description: "A short description of the start page.", host: "site.test", breadcrumb: "› start", favicon: null },
  2: { address: "https://site.test/w29", title: "W".repeat(29), description: "a".repeat(126), host: "site.test", breadcrumb: "› w29", favicon: null },
  3: { address: "https://site.test/w30", title: "W".repeat(30), description: "a".repeat(127), host: "site.test", breadcrumb: "› w30", favicon: null },
  4: { address: "https://site.test/amp", title: "Tom & Jerry", description: "", host: "site.test", breadcrumb: "› amp", favicon: null },
  5: { address: "https://site.test/emoji", title: "Über uns 😀 Käse Straße", description: "Ünïcödé 😀 text", host: "site.test", breadcrumb: "› emoji", favicon: null },
};
export const ORIGINAL_HTML = ["<!doctype html>", '<html lang="en">', "<head>", "<title>Start</title>", "</head>", "<body>", "<h1>Welcome</h1>", "<p>Hello world &amp; friends</p>", "</body>", "</html>"];
export const RENDERED_HTML = ["<!doctype html>", '<html lang="en">', "<head>", "<title>Start - rendered</title>", "</head>", "<body>", "<h1>Welcome</h1>", "<p>Hello world &amp; friends</p>", '<p id="late">Added by script today</p>', "</body>", "</html>"];
export const ORIGINAL_TEXT = ["Welcome Hello world & friends"];
export const RENDERED_TEXT = ["Welcome Hello world & friends Added by script today"];

export const SD_NODES: SdNode[] = [
  { id: 1, format: "json-ld", typePath: "Product", props: [{ name: "name", value: "Shoe" }, { name: "brand", value: "Acme" }], propertyOrder: ["name", "offers", "brand"], path: "", sourceLine: 1, sourceCol: 1,
    issues: [{ severity: "warning", validation: "schema.org", feature: null, property: "brand", code: "sd.schema.expected_object", message: "Property `brand` should be an object.", path: "brand", line: null, col: null, snippet: null, docUrl: "https://schema.org/brand" }],
    children: [{ id: 2, format: "json-ld", typePath: "Offer", props: [{ name: "price", value: "9.99" }], propertyOrder: ["price"], path: "offers[0]", children: [], sourceLine: 5, sourceCol: 3,
      issues: [{ severity: "error", validation: "google", feature: "Merchant listing", property: "offers[0].priceCurrency", code: "sd.google.missing_required", message: "Property `priceCurrency` is missing (required for Merchant listing).", path: "offers[0].priceCurrency", line: null, col: null, snippet: null, docUrl: "https://developers.google.com/search/docs/appearance/structured-data/merchant-listing" }] }] },
  { id: 3, format: "json-ld", typePath: "Parse Error", props: [], children: [], propertyOrder: [], path: "", sourceLine: 3, sourceCol: 7,
    issues: [{ severity: "error", validation: "schema.org", feature: null, property: null, code: "sd.parse_error", message: "The script block contains invalid JSON.", path: "", line: 3, col: 7, snippet: '{"@type": "Pro, "name": "Broken"}', docUrl: null }] },
  { id: 4, format: "microdata", typePath: "Person", props: [{ name: "name", value: "Ada" }, { name: "jobTitle", value: "Engineer" }], children: [], propertyOrder: ["name", "jobTitle"], path: "", sourceLine: 20, sourceCol: 1, issues: [] },
  { id: 5, format: "rdfa", typePath: "Article", props: [{ name: "headline", value: "News" }], children: [], propertyOrder: ["headline"], path: "", sourceLine: 30, sourceCol: 1, issues: [] },
];
export const DUPLICATES = {
  exact: { 10: { text: "Alpha beta gamma delta epsilon", partners: [{ urlId: 12, address: "https://site.test/a?ref=x", similarity: 100 }, { urlId: 11, address: "https://site.test/b.html", similarity: 100 }] },
    11: { text: "Alpha beta gamma delta epsilon", partners: [] }, 12: { text: "Alpha beta gamma delta epsilon", partners: [] } } as Record<number, { text: string; partners: { urlId: number; address: string; similarity: number }[] }>,
  near: { 20: { text: "The quick brown fox jumps over the lazy dog today", partners: [{ urlId: 22, address: "https://site.test/near-91", similarity: 91 }, { urlId: 21, address: "https://site.test/near-95", similarity: 95 }] },
    21: { text: "The quick brown fox leaps over the lazy dog today", partners: [] }, 22: { text: "A quick brown fox leaps over the very lazy dog", partners: [] } } as Record<number, { text: string; partners: { urlId: number; address: string; similarity: number }[] }>,
};
export const SPELLING = [
  { Error: "Kontackt", Type: "Spelling", Detail: "Possible spelling mistake", Suggestion: "Kontakt", "Page Section": "Page Title", "Rule ID": "", context: { text: "Kontackt", start: 0, end: 8, suggestions: ["Kontakt"] } },
  { Error: "Rechtschreibfeler", Type: "Spelling", Detail: "Possible spelling mistake", Suggestion: "Rechtschreibfehler", "Page Section": "Body", "Rule ID": "",
    context: { text: "Ein Rechtschreibfeler und noch ein Rechtschreibfeler im Text", start: 4, end: 21, suggestions: ["Rechtschreibfehler"] } },
  { Error: "ein ein", Type: "Grammar", Detail: "Repeated word", Suggestion: "ein", "Page Section": "Body", "Rule ID": "de.word-repeat", context: { text: "Das ist ein ein Test", start: 8, end: 15, suggestions: ["ein"] } },
];
export const LONG_MESSAGE = "x".repeat(10_050);
export const CONSOLE = [
  { Level: "error", Message: "Uncaught Error: fixture-error", "Source URL": "https://site.test/app.js", Line: 3, Column: 7, Timestamp: 120 },
  { Level: "warning", Message: "deprecated call", "Source URL": "https://site.test/app.js", Line: 9, Column: 1, Timestamp: 130 },
  { Level: "log", Message: LONG_MESSAGE, "Source URL": "https://site.test/app.js", Line: 12, Column: 2, Timestamp: 150 },
];
export const BLOCKED = [{ Address: "https://site.test/blocked.css", Type: "CSS", "Status Code": 0, "Blocked By": "robots.txt" }];
export const NETWORK = [...BLOCKED, { Address: "https://site.test/app.js", Type: "JavaScript", "Status Code": 200, "Blocked By": "" }, { Address: "https://site.test/logo.png", Type: "Image", "Status Code": 200, "Blocked By": "" }];
export const A11Y_RULES = [
  { Issue: "heading-order", Result: "Violation", Guideline: "Best practice", Impact: "Moderate", Nodes: 3, "Help URL": "https://dequeuniversity.com/rules/axe/heading-order" },
  { Issue: "image-alt", Result: "Violation", Guideline: "WCAG 2.0 A", Impact: "Critical", Nodes: 2, "Help URL": "https://dequeuniversity.com/rules/axe/image-alt" },
  { Issue: "color-contrast", Result: "Incomplete", Guideline: "WCAG 2.0 AA", Impact: "Serious", Nodes: 1, "Help URL": "https://dequeuniversity.com/rules/axe/color-contrast" },
  { Issue: "video-caption", Result: "Inapplicable", Guideline: "WCAG 2.0 A", Impact: "", Nodes: "", "Help URL": "https://dequeuniversity.com/rules/axe/video-caption" },
  { Issue: "html-has-lang", Result: "Passed", Guideline: "WCAG 2.0 A", Impact: "", Nodes: "", "Help URL": "https://dequeuniversity.com/rules/axe/html-has-lang" },
];
export const A11Y_NODES: Record<string, { id: number; Address: string; "Location on Page": string; HTML: string }[]> = {
  "image-alt": [{ id: 101, Address: "https://site.test/", "Location on Page": "img.hero", HTML: '<img class="hero" src="a.png">' }, { id: 102, Address: "https://site.test/", "Location on Page": "#late > img", HTML: '<img src="b.png">' }],
  "heading-order": [{ id: 103, Address: "https://site.test/", "Location on Page": "h4", HTML: "<h4>Skipped</h4>" }],
  "color-contrast": [{ id: 104, Address: "https://site.test/", "Location on Page": "p.hero", HTML: '<p class="hero">Text over image</p>' }],
};

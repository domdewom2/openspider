import { parse } from "@fluent/syntax";
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { checkCatalog, checkIdenticalGerman } from "./check";
import { loadFtlFiles } from "./index";
import type { FtlFile } from "./core";

const glossary = readFileSync("../../docs/plan/M2-acceptance/beta/begriffe.md", "utf8").split("## Ausnahmen")[1]!;
const exceptions = new Set([...glossary.matchAll(/^\| `([^`]+)`/gm)].map(match => match[1]!));
// Additional equal German words, data formats, product names and units outside U34's tab glossary.
// Every value is explicit: a new identical English phrase must receive a translation or a reviewed exception.
const equalGerman = ["OK", "Orange", "Name", "Segment", "System", "Zoom", "Crawls", "Operator", "Format", "Spider", "Cookies", "Text", "Regex", "Details", "Server", "Port", "Filter", "Standard", "Export", "Version", "Cache", "Subdomain", "Desktop", "Tablet", "Iframes"];
const technical = ["XPath", "User-Agent", "Accept-Language", "KB", "ms", "MB", "px", "hreflang", "Hash (MD5)", "HTTP/1.1", "PNG", "JPEG", "Google Analytics 4", "GSC", "PSI", "Drive", "APIs", "SVG", "OPENSPIDER_HOME", "JavaScript…", "SERP-Snippet", "NoODP", "NoYDIR", "WCAG 2.0 A", "WCAG 2.0 AA", "WCAG 2.0 AAA", "WCAG 2.1 AA", "WCAG 2.2 AA"];
const dataTemplates = ["s", "URLs", "/  URLs", "URLs/s", "API:  %", "+ / −", "Export:", "Host:  · Realm:", "PNG ×", "Version"];
for (const value of [...equalGerman, ...technical, ...dataTemplates]) exceptions.add(value);

// spec: UI-121
it("keeps de/en complete across every feature and reports unreviewed identical German values in CI (K2-75)", () => {
  const files = loadFtlFiles();
  expect(checkCatalog(files).problems).toEqual([]);
  expect(checkIdenticalGerman(files, exceptions)).toEqual([]);
});

// spec: UI-121
it("detects copied text, attributes and plurals while permitting glossary terms and pure data", () => {
  const source = "copied = Save\n    .title = Open\nplural = { $n ->\n [one] One item\n *[other] Many items\n}\nterm = HTTP\ndata = { $n }\n";
  const file = (lang: string): FtlFile => ({ path: `${lang}.ftl`, lang, feature: "fixture", source });
  const hits = checkIdenticalGerman([file("de"), file("en")], new Set(["HTTP"]));
  expect(hits).toHaveLength(3);
  expect(hits.join("\n")).toContain("copied.title");
  expect(hits.join("\n")).toContain("plural");
});

// spec: UI-121
it("keeps every canonical column a complete literal in de/en for the CLI build (K2-78)", () => {
  const files = loadFtlFiles().filter(f => f.feature === "i18n/canon");
  const columns = (lang: string) => {
    const body = parse(files.find(f => f.lang === lang)!.source, { withSpans: false }).body;
    expect(body.filter(entry => entry.type === "Junk")).toEqual([]);
    const messages = body.filter(entry => entry.type === "Message" && entry.id.name.startsWith("column-"));
    expect(messages.length).toBeGreaterThan(0);
    for (const message of messages) {
      if (message.type !== "Message") throw new Error("Expected message");
      expect(message.attributes, message.id.name).toEqual([]);
      expect(message.value?.elements.every(element => element.type === "TextElement"), message.id.name).toBe(true);
      expect(message.value?.elements.map(element => element.type === "TextElement" ? element.value : "").join("").trim(), message.id.name).not.toBe("");
    }
    return messages.map(message => message.type === "Message" ? message.id.name : "").sort();
  };
  expect(columns("de")).toEqual(columns("en"));
});

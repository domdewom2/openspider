import { describe, expect, it, vi } from "vitest";
import { getFormatters, makeT } from "../../../i18n";
import type { OverviewDto } from "../../../ipc/gen/OverviewDto";
import { THEME_TOKENS } from "../../../ui/tokens/tokens";
import fxChart from "../../../../../../tests/fixtures/ui/fx-ui-chart/overview.json";
import fxUnassigned from "../../../../../../tests/fixtures/ui/fx-ui-chart/overview-unassigned.json";
import fxOverflow from "../../../../../../tests/fixtures/ui/fx-ui-chart/overview-overflow.json";
import { dataRows, fileName, toCsv, toTsv } from "./data";
import { buildChart, highlightKey, legendOrder, pctOf, type ChartCategory, type ChartModel } from "./model";
import { chartEntry } from "../overview/selection";
import { chartHeight, defaultChartHeight, finishDrag } from "./pane";
import { DEFAULT_CHART_LAYOUT } from "../frame/layout";
import { chartSvg, pngSize, ringSegments, svgSize } from "./svg";

const data = fxChart as unknown as OverviewDto;
const de = { t: makeT("de"), f: getFormatters("de") };
const en = { t: makeT("en"), f: getFormatters("en") };
const pct = (lang: typeof de, c: ChartCategory) => `${lang.f.decimal(c.pct!, 1)} %`;
type Chart = Extract<ChartModel, { kind: "chart" }>;
function chart(d: OverviewDto, entry: Parameters<typeof buildChart>[1], lang = de, log?: (e: string, s: number, b: number) => void): Chart {
  const m = buildChart(d, entry, lang.t, log);
  if (m.kind !== "chart") throw new Error(`expected chart, got ${m.kind}`);
  return m;
}
const rows = (cs: readonly ChartCategory[], lang = de) => cs.map(c => [c.canonical, lang.f.integer(c.count!), pct(lang, c)]);

describe("UI-102 chart model on fx-ui-chart", () => {
  // spec: UI-102
  it("Response Codes: ring of exactly six segments, Other only in the legend, meta refresh and chains outside", () => {
    const m = chart(data, { kind: "tab", tab: "Response Codes" });
    expect(m.chart).toBe("ring");
    expect(rows(m.drawn)).toEqual([
      ["Success (2XX)", "812", "81,2 %"], ["Redirection (3XX)", "96", "9,6 %"], ["Client Error (4XX)", "61", "6,1 %"],
      ["Blocked by Robots.txt", "17", "1,7 %"], ["Server Error (5XX)", "9", "0,9 %"], ["No Response", "5", "0,5 %"],
    ]);
    expect(m.tail.map(c => [c.canonical, c.count])).toEqual([["Other", 0]]);
    const all = [...m.drawn, ...m.tail, ...m.table].map(c => c.canonical);
    expect(all).not.toContain("Redirect Chain");
    expect(all).not.toContain("Redirection (Meta Refresh)");
    expect(`${de.f.integer(m.header.base)} URLs`).toBe("1.000 URLs");
    expect(`${en.f.integer(m.header.base)} URLs`).toBe("1,000 URLs");
    expect(en.t("ovc-urls", { count: en.f.integer(m.header.base) })).toBe("1,000 URLs");
    // Status class colours; the second category of a colour is hatched (4XX/5XX, blocked/no response/other).
    const byName = new Map([...m.drawn, ...m.tail].map(c => [c.canonical, c]));
    // Orchestrator decision 03.11.2026: 2XX calm green instead of the near-black status.2xx; never the text colour.
    expect(byName.get("Success (2XX)")).toMatchObject({ color: "diff.add", hatch: 0 });
    expect(byName.get("Redirection (3XX)")).toMatchObject({ color: "status.3xx", hatch: 0 });
    for (const theme of ["light", "dark"] as const) expect(THEME_TOKENS[theme]["diff.add"]).not.toBe(THEME_TOKENS[theme].text);
    expect(en.t("ovc-cat-others")).toBe("Remaining");
    expect(byName.get("Client Error (4XX)")).toMatchObject({ color: "status.4xx", hatch: 0 });
    expect(byName.get("Server Error (5XX)")).toMatchObject({ color: "status.5xx", hatch: 1 });
    expect(THEME_TOKENS.light["status.4xx"]).toBe(THEME_TOKENS.light["status.5xx"]);
    expect(byName.get("Blocked by Robots.txt")).toMatchObject({ color: "status.none", hatch: 0 });
    expect(byName.get("No Response")).toMatchObject({ color: "status.none", hatch: 1 });
  });

  // spec: UI-102
  it("Internal: ten bars with „Weitere“ bundling XML and Flash, table with all eleven categories", () => {
    const m = chart(data, { kind: "tab", tab: "Internal" });
    expect(m.chart).toBe("bars");
    expect(m.drawn).toHaveLength(10);
    expect(m.drawn.map(c => [c.canonical, de.f.integer(c.count!), pct(de, c)])).toEqual([
      ["HTML", "1.200", "60,0 %"], ["Images", "400", "20,0 %"], ["JavaScript", "180", "9,0 %"], ["CSS", "90", "4,5 %"], ["PDF", "30", "1,5 %"],
      ["Other", "26", "1,3 %"], ["Unknown", "24", "1,2 %"], ["Fonts", "20", "1,0 %"], ["Media", "14", "0,7 %"], ["Remaining", "16", "0,8 %"],
    ]);
    expect(m.drawn.map(c => c.canonical)).toEqual(["HTML", "Images", "JavaScript", "CSS", "PDF", "Other", "Unknown", "Fonts", "Media", "Remaining"]);
    const others = m.drawn.at(-1)!;
    expect(others).toMatchObject({ role: "others", label: "Weitere", count: 16, color: "chart.other", nav: null });
    expect(others.members!.map(c => [c.canonical, c.count])).toEqual([["XML", 10], ["Flash", 6]]);
    expect(m.table).toHaveLength(11);
    expect(m.table.some(c => c.role === "others")).toBe(false);
    // Legend order with „Weitere“ expanded: the members follow it.
    expect(legendOrder(m, true).slice(-3).map(c => c.canonical)).toEqual(["Remaining", "XML", "Flash"]);
    // Nine palette colours for eleven categories: the repeated colours are hatched.
    const color = (name: string) => m.table.find(c => c.canonical === name)!.color;
    expect(color("Media")).toBe(color("HTML"));
    expect(m.table.filter(c => c.hatch > 0).map(c => c.canonical).sort()).toEqual(["Fonts", "Media", "XML"]);
  });

  // spec: UI-102
  it("Summary: ring of five segments whose percentages equal the Overview's summary column character by character", () => {
    const m = chart(data, { kind: "summary" });
    expect(m.chart).toBe("ring");
    expect(m.drawn.map(c => [c.label, de.f.integer(c.count!), pct(de, c)])).toEqual([
      ["Intern indexierbar", "1.100", "44,0 %"], ["Intern ohne Indexierbarkeitsstatus", "800", "32,0 %"], ["Extern", "400", "16,0 %"],
      ["Intern nicht indexierbar", "100", "4,0 %"], ["Nicht angezeigt", "100", "4,0 %"],
    ]);
    // Overview summary column: count / Total URLs Encountered × 100, formatted with one decimal (Overview.tsx).
    const encountered = data.summary[0]!.count;
    const overview = (key: string) => `${de.f.decimal(data.summary.find(r => r.key === key)!.count / encountered * 100, 1)} %`;
    const byKey = new Map(m.drawn.map(c => [c.key, pct(de, c)]));
    expect(byKey.get("indexable")).toBe(overview("Total Internal Indexable URLs"));
    expect(byKey.get("nonIndexable")).toBe(overview("Total Internal Non-Indexable URLs"));
    expect(byKey.get("external")).toBe(overview("Total External URLs"));
    expect(m.drawn.find(c => c.key === "hidden")).toMatchObject({ role: "hidden", nav: null, color: "chart.rest" });
  });

  // spec: UI-102
  it("Page Titles: bars without partition, descending, no „Weitere“, one filter without matches, issue colours", () => {
    const m = chart(data, { kind: "tab", tab: "Page Titles" }, en);
    expect(m.chart).toBe("overlap");
    expect(rows(m.drawn, en)).toEqual([
      ["Over {title.max_chars} Characters", "300", "25.0 %"], ["Duplicate", "240", "20.0 %"], ["Below {title.min_chars} Characters", "60", "5.0 %"],
      ["Missing", "12", "1.0 %"], ["Multiple", "6", "0.5 %"],
    ].map(([n, ...r]) => [n!.replace("{title.max_chars}", "60").replace("{title.min_chars}", "30"), ...r]));
    expect(m.drawn.map(c => c.label)).toEqual(["Over 60 Characters", "Duplicate", "Below 30 Characters", "Missing", "Multiple"]);
    expect(m.drawn.some(c => c.role === "others")).toBe(false);
    expect(m.zero.map(c => c.canonical)).toEqual(["Same as H1"]);
    expect(en.t("ovc-zero-filters", { count: m.zero.length })).toBe("1 filter without matches");
    expect(de.t("ovc-zero-filters", { count: 1 })).toBe("1 Filter ohne Treffer");
    expect(en.t("ovc-overlap-note")).toBe("A URL can be in several categories – the shares do not add up to 100 %.");
    expect(de.t("ovc-overlap-note")).toBe("Eine URL kann in mehreren Kategorien stehen – die Anteile ergeben zusammen nicht 100 %.");
    // Priority colour and type as in the Overview badge.
    expect(m.drawn.find(c => c.canonical === "Missing")).toMatchObject({ color: "issue.prio.high", issue: { issueType: "issue", priority: "high" } });
    expect(m.drawn.find(c => c.canonical === "Duplicate")).toMatchObject({ color: "issue.prio.medium", issue: { issueType: "opportunity" } });
  });

  // spec: UI-102
  it("residual category: All = 1 010 adds „Nicht zugeordnet“ 10 (1,0 %) without a target; sum > base gives bars and a log warning", () => {
    const m = chart(fxUnassigned as unknown as OverviewDto, { kind: "tab", tab: "Response Codes" });
    expect(m.chart).toBe("ring");
    const rest = m.drawn.find(c => c.role === "unassigned")!;
    expect([rest.label, rest.count, pct(de, rest), rest.nav, rest.color]).toEqual(["Nicht zugeordnet", 10, "1,0 %", null, "chart.rest"]);
    expect(m.drawn).toHaveLength(7);
    expect(m.table.filter(c => c.role === "unassigned")).toHaveLength(1);
    const log = vi.fn();
    const over = chart(fxOverflow as unknown as OverviewDto, { kind: "tab", tab: "Response Codes" }, de, log);
    expect(over.chart).toBe("overlap");
    expect(over.overflow).toBe(true);
    expect(over.drawn.some(c => c.role === "others" || c.role === "unassigned")).toBe(false);
    expect(log).toHaveBeenCalledWith("Response Codes", 1000, 990);
  });

  // spec: UI-102
  it("without counters: a block with only pending filters gives the hint, a mixed block draws only counted categories", () => {
    const pending = buildChart(data, { kind: "tab", tab: "Sitemaps" }, de.t);
    expect(pending.kind).toBe("noCounts");
    const mixed = chart(data, { kind: "tab", tab: "Canonicals" });
    expect(mixed.drawn.every(c => c.count != null && c.count > 0)).toBe(true);
    expect(mixed.tail.map(c => [c.canonical, c.count, c.state?.kind])).toEqual([["Unlinked", null, "needsCrawlAnalysis"], ["Non-Indexable Canonical", null, "needsCrawlAnalysis"]]);
    const csv = dataRows(mixed);
    expect(csv.find(r => r[1] === "Unlinked")).toEqual(["Canonicals", "Unlinked", "Unlinked", null, null, 1200]);
  });

  // spec: UI-102
  it("a selected filter row shows its block's chart and highlights its category; All highlights nothing", () => {
    const entry = { kind: "filter", tab: "Response Codes", filter: "Client Error (4XX)" } as const;
    const m = buildChart(data, chartEntry(entry), de.t);
    expect(highlightKey(m, entry)).toBe("f:Client Error (4XX)");
    expect(highlightKey(m, { kind: "filter", tab: "Response Codes", filter: "All" })).toBeNull();
    expect(highlightKey(m, { kind: "filter", tab: "Response Codes", filter: "Redirect Chain" })).toBeNull();
    const internal = buildChart(data, { kind: "tab", tab: "Internal" }, de.t);
    expect(highlightKey(internal, { kind: "filter", tab: "Internal", filter: "XML" })).toBe("others");
  });

  // spec: UI-102
  it("click targets: categories name tab and filter, summary categories add the indexability condition", () => {
    const rc = chart(data, { kind: "tab", tab: "Response Codes" });
    expect(rc.drawn.find(c => c.canonical === "Client Error (4XX)")!.nav).toEqual({ tab: "Response Codes", filter: "Client Error (4XX)" });
    const summary = chart(data, { kind: "summary" });
    const nav = (key: string) => summary.drawn.find(c => c.key === key)!.nav;
    const rule = (op: string, value: string | null) => ({ kind: "rules", caseSensitive: false, tree: { kind: "group", op: "and", children: [{ kind: "cond", column: "Indexability", op, value }] } });
    expect(nav("indexable")).toEqual({ tab: "Internal", filter: "HTML", search: rule("equals", "Indexable") });
    expect(nav("nonIndexable")).toEqual({ tab: "Internal", filter: "HTML", search: rule("equals", "Non-Indexable") });
    expect(nav("noStatus")).toEqual({ tab: "Internal", filter: "All", search: rule("isEmpty", null) });
    expect(nav("external")).toEqual({ tab: "External", filter: "All" });
    expect(nav("hidden")).toBeNull();
  });

  // spec: UI-102
  it("empty states: a base of 0 and an entry that disappeared", () => {
    const empty = structuredClone(data);
    empty.tabs.find(t => t.tab === "Internal")!.filters.forEach(f => { f.count = 0; f.pct = 0; });
    expect(buildChart(empty, { kind: "tab", tab: "Internal" }, de.t).kind).toBe("zero");
    expect(buildChart(data, { kind: "tab", tab: "Nope" }, de.t).kind).toBe("missing");
    expect(de.t("ovc-empty")).toBe("Eintrag in der Übersicht auswählen, um seine Verteilung zu sehen.");
    expect(en.t("ovc-empty")).toBe("Select an entry in the overview to see its breakdown.");
    expect(de.t("ovc-zero")).toBe("Noch keine URLs in diesem Eintrag.");
    expect(en.t("ovc-zero")).toBe("No URLs in this entry yet.");
  });

  // spec: UI-102
  it("percentages use the core's rounding (tenths, halves up)", () => {
    expect(pctOf(3, 7)).toBe(42.9);
    expect(pctOf(1, 200)).toBe(0.5);
    expect(pctOf(10, 1010)).toBe(1);
    expect(pctOf(0, 0)).toBe(0);
  });
});

describe("UI-102 exports", () => {
  // spec: UI-102
  it("CSV of Internal: header and eleven rows with the table values and Base 2000; copy gives the same rows tab separated", () => {
    const m = chart(data, { kind: "tab", tab: "Internal" });
    const table = dataRows(m);
    expect(table[0]).toEqual(["Entry", "Category", "Filter", "URLs", "% of Total", "Base"]);
    expect(table).toHaveLength(12);
    expect(table.slice(1).every(r => r[5] === 2000 && r[0] === "Internal")).toBe(true);
    expect(table.slice(1).map(r => [r[1], r[3]])).toEqual(m.table.map(c => [c.canonical, c.count]));
    expect(table[1]).toEqual(["Internal", "HTML", "HTML", 1200, "60.0", 2000]);
    const csv = toCsv(table);
    expect(csv.startsWith("﻿Entry,Category,Filter,URLs,% of Total,Base\r\n")).toBe(true);
    expect(csv.split("\r\n").filter(Boolean)).toHaveLength(12);
    expect(csv).toContain("\r\nInternal,HTML,HTML,1200,60.0,2000\r\n");
    expect(csv).toContain("\r\nInternal,Media,Media,14,0.7,2000\r\n");
    expect(table.slice(1).every(r => typeof r[4] === "string" && /^\d+\.\d$/.test(r[4]))).toBe(true);
    const tsvText = toTsv(table);
    expect(tsvText.split("\n")).toHaveLength(12);
    expect(tsvText.split("\n")[2]).toBe(["Internal", "Images", "Images", "400", "20.0", "2000"].join("\t"));
    // Residual categories: empty Filter; without counter: empty URLs.
    const rest = dataRows(chart(fxUnassigned as unknown as OverviewDto, { kind: "tab", tab: "Response Codes" }));
    expect(rest.find(r => r[1] === "Unassigned")).toEqual(["Response Codes", "Unassigned", null, 10, "1.0", 1010]);
    expect(toCsv(rest)).toContain("\r\nResponse Codes,Unassigned,,10,1.0,1010\r\n");
    // Option „column names in UI language“.
    expect(dataRows(m, { uiHeaders: true, t: de.t })[0]).toEqual(["Eintrag", "Kategorie", "Filter", "URLs", "% gesamt", "Grundmenge"]);
    expect(fileName("Response Codes", "Shop A", "csv", new Date(2026, 10, 3))).toBe("overview-response-codes-shop-a-2026-11-03.csv");
    expect(fileName("Summary", null, "png", new Date(2026, 0, 9))).toBe("overview-summary-2026-01-09.png");
  });

  // spec: UI-102
  it("SVG export is SVG 1.1 with texts, hex colours and a <pattern> for the second category of a colour; PNG 2× doubles 1×", () => {
    const m = chart(data, { kind: "tab", tab: "Response Codes" }, en);
    const hex = (token: keyof typeof THEME_TOKENS.light) => THEME_TOKENS.light[token].toLowerCase();
    const svg = chartSvg({
      kind: "ring", title: "Response Codes – 1,000 URLs", center: ["1,000", "URLs"],
      items: m.drawn.map(c => ({ label: c.label, count: en.f.integer(c.count!), pct: pct(en, c), color: hex(c.color), hatch: c.hatch, value: c.count! })),
      tail: m.tail.map(c => ({ label: c.label, count: "0", pct: "0.0 %" })),
      colors: { surface: "#ffffff", text: "#1b1f24", muted: "#59626d", focus: "#1f62c4", track: "#f6f7f9" }, font: "sans-serif", width: 360, height: 200,
    });
    const doc = new DOMParser().parseFromString(svg, "image/svg+xml");
    expect(doc.getElementsByTagName("parsererror")).toHaveLength(0);
    const root = doc.documentElement;
    expect([root.namespaceURI, root.getAttribute("version")]).toEqual(["http://www.w3.org/2000/svg", "1.1"]);
    const allowed = new Set(["svg", "defs", "pattern", "rect", "g", "path", "text"]);
    for (const el of Array.from(doc.getElementsByTagName("*"))) {
      expect(allowed.has(el.localName), el.localName).toBe(true);
      for (const a of ["fill", "stroke"]) {
        const v = el.getAttribute(a);
        if (v !== null && v !== "none") expect(v, `${el.localName} ${a}`).toMatch(/^(#[0-9a-f]{6}|url\(#hatch-\d+\))$/);
      }
    }
    const texts = Array.from(doc.getElementsByTagName("text")).map(t => t.textContent);
    expect(texts).toEqual(expect.arrayContaining(["Client Error (4XX)", "Server Error (5XX)", "61 (6.1 %)", "9 (0.9 %)", "1,000"]));
    const segments = Array.from(doc.querySelectorAll("path.segment"));
    expect(segments).toHaveLength(6);
    const fill = (label: string) => segments[m.drawn.findIndex(c => c.label === label)]!.getAttribute("fill")!;
    expect(fill("Client Error (4XX)")).toBe(hex("status.4xx"));
    expect(fill("Server Error (5XX)")).toMatch(/^url\(#hatch-\d+\)$/);
    const pattern = doc.getElementById(fill("Server Error (5XX)").slice(5, -1))!;
    expect(pattern.localName).toBe("pattern");
    expect(pattern.querySelector("rect")!.getAttribute("fill")).toBe(hex("status.4xx"));
    const size = svgSize(svg);
    expect(size.width).toBe(360);
    const one = pngSize(size.width, size.height, 1, 2);
    const two = pngSize(size.width, size.height, 2, 2);
    expect([two.width, two.height]).toEqual([one.width * 2, one.height * 2]);
    expect(one.width).toBe(720);
  });

  // spec: UI-102
  it("ring geometry: clockwise from 12 o'clock, a single category is a full ring", () => {
    const [a, b] = ringSegments([3, 1]);
    expect(a!.a0).toBe(0);
    expect(a!.a1).toBeCloseTo(Math.PI * 1.5);
    expect(b!.a1).toBeCloseTo(Math.PI * 2);
    expect(ringSegments([5])[0]!.d.match(/A /g)).toHaveLength(4);
  });
});

describe("UI-102 layout of the chart area", () => {
  // spec: UI-102
  it("default height max(180 px, 40 %), minimum 140 px, under 70 px collapsed, restore keeps the last share", () => {
    expect(defaultChartHeight(800)).toBe(320);
    expect(defaultChartHeight(300)).toBe(180);
    expect(chartHeight(800, DEFAULT_CHART_LAYOUT)).toBe(320);
    expect(chartHeight(800, { ...DEFAULT_CHART_LAYOUT, share: 0.1 })).toBe(140);
    const collapsed = finishDrag({ ...DEFAULT_CHART_LAYOUT, share: 0.5 }, 60, 800);
    expect(collapsed).toEqual({ collapsed: true });
    expect(chartHeight(800, { ...DEFAULT_CHART_LAYOUT, share: 0.5, ...collapsed })).toBe(0);
    expect(chartHeight(800, { ...DEFAULT_CHART_LAYOUT, share: 0.5, collapsed: false })).toBe(400);
    expect(finishDrag(DEFAULT_CHART_LAYOUT, 100, 800)).toEqual({ collapsed: false, share: 140 / 800 });
    expect(finishDrag(DEFAULT_CHART_LAYOUT, 400, 1000)).toEqual({ collapsed: false, share: 0.4 });
  });
});

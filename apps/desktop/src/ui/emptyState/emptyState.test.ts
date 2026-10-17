import { describe, expect, it } from "vitest";
import { EMPTY_CODES, EMPTY_RANK, emptyMessageKey, emptyStateAttr, pickEmptyState } from "./codes";
import { evaluatePrerequisites, findMissingRegisterEntries, type RegisterEntry } from "./register";

describe("Leerzustände (UI-016)", () => {
  // spec: UI-016
  it("kennt genau die elf abschließenden Codes mit den Rängen der Spec", () => {
    expect(EMPTY_CODES).toHaveLength(11);
    expect(EMPTY_RANK.NO_SELECTION).toBe(1);
    expect(EMPTY_RANK.NEEDS_JS_RENDERING).toBe(3);
    expect(EMPTY_RANK.FEATURE_DISABLED).toBe(4);
    expect(EMPTY_RANK.NO_MATCH).toBe(9);
    expect(EMPTY_RANK.NOT_APPLICABLE).toBe(10);
  });

  // spec: UI-016
  it("bei mehreren Ursachen gewinnt der kleinste Rang; NO_MATCH nur ohne Ursache mit kleinerem Rang", () => {
    const picked = pickEmptyState([
      { code: "NO_MATCH" },
      { code: "FEATURE_DISABLED", arg: "accessibility" },
      { code: "NEEDS_JS_RENDERING" },
    ]);
    expect(picked?.code).toBe("NEEDS_JS_RENDERING");
    expect(pickEmptyState([{ code: "NO_MATCH" }, { code: "NO_DATA_YET" }])?.code).toBe("NO_DATA_YET");
    expect(pickEmptyState([])).toBeNull();
  });

  // spec: UI-016
  it("data-empty-state trägt Code und Argument: FEATURE_DISABLED(cookies)", () => {
    expect(emptyStateAttr({ code: "FEATURE_DISABLED", arg: "cookies" })).toBe("FEATURE_DISABLED(cookies)");
    expect(emptyStateAttr({ code: "NEEDS_CRAWL_ANALYSIS", arg: "canonicals" })).toBe("NEEDS_CRAWL_ANALYSIS(canonicals)");
    expect(emptyStateAttr({ code: "NO_SELECTION" })).toBe("NO_SELECTION");
    expect(emptyMessageKey("HTML_NOT_STORED")).toBe("empty-html-not-stored");
  });

  const register: RegisterEntry[] = [
    { kind: "detailTab", id: "View Source", conditions: [{ code: "HTML_NOT_STORED", action: { kind: "openSetting", anchor: "extraction.store_html" } }] },
    { kind: "detailTab", id: "Rendered Page", conditions: [{ code: "NEEDS_JS_RENDERING", action: { kind: "openSetting", anchor: "rendering.mode" } }] },
    { kind: "detailTab", id: "Accessibility Details", conditions: [{ code: "NEEDS_JS_RENDERING" }, { code: "FEATURE_DISABLED", arg: "accessibility" }] },
    { kind: "filter", id: "Canonicals:Unlinked", conditions: [{ code: "NEEDS_CRAWL_ANALYSIS", arg: "canonicals", action: { kind: "startAnalysis" } }] },
    { kind: "topTab", id: "Search Console", conditions: [{ code: "API_NOT_CONNECTED", arg: "gsc", action: { kind: "connectApi", api: "gsc" } }] },
    { kind: "filter", id: "Internal:All", conditions: [] },
  ];

  // spec: UI-016
  it("Standard-Konfiguration: erwartete Codes nach der Liste der Spec, zweiter Lauf mit JS-Rendering an und Accessibility aus", () => {
    const standard = (p: { code: string }) => p.code !== "NO_SELECTION";
    const byId = (id: string) => register.find((e) => e.id === id) as RegisterEntry;
    expect(evaluatePrerequisites(byId("View Source"), standard)?.code).toBe("HTML_NOT_STORED");
    expect(evaluatePrerequisites(byId("Rendered Page"), standard)?.code).toBe("NEEDS_JS_RENDERING");
    expect(evaluatePrerequisites(byId("Canonicals:Unlinked"), standard)).toMatchObject({ code: "NEEDS_CRAWL_ANALYSIS", arg: "canonicals" });
    expect(evaluatePrerequisites(byId("Search Console"), standard)).toMatchObject({ code: "API_NOT_CONNECTED", arg: "gsc" });
    expect(evaluatePrerequisites(byId("Accessibility Details"), standard)?.code).toBe("NEEDS_JS_RENDERING");
    // JS-Rendering an, Accessibility aus → Rang 4 statt 3
    const jsOn = (p: { code: string }) => p.code === "FEATURE_DISABLED";
    expect(evaluatePrerequisites(byId("Accessibility Details"), jsOn)).toMatchObject({ code: "FEATURE_DISABLED", arg: "accessibility" });
    // Voraussetzungen erfüllt → kein Hinweis
    expect(evaluatePrerequisites(byId("Internal:All"), () => true)).toBeNull();
    expect(evaluatePrerequisites(byId("View Source"), () => false)).toBeNull();
  });

  // spec: UI-016
  it("CI-Prüfung: fehlende Einträge im Voraussetzungsregister werden gemeldet (leere Liste zählt als Eintrag)", () => {
    const required = [
      { kind: "filter" as const, id: "internal:all" },
      { kind: "detailTab" as const, id: "Cookies" },
      { kind: "sidebarTab" as const, id: "Overview" },
    ];
    expect(findMissingRegisterEntries(register, required)).toEqual(["detailTab:Cookies", "sidebarTab:Overview"]);
    expect(findMissingRegisterEntries([...register, { kind: "detailTab", id: "Cookies", conditions: [] }], required.slice(0, 2))).toEqual([]);
  });
});

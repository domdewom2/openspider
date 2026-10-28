import { afterEach, describe, expect, it } from "vitest";
import { canonKey, columnLabel, filterLabel, tabLabel, thresholdOf } from "./canon";
import { getT, i18n, setLanguage } from "./index";

afterEach(async () => { await setLanguage("en"); });

describe("canonical names in the UI language (A02-04)", () => {
  // spec: UI-121
  it("derives Fluent identifiers from canonical names without dots, braces or brackets", () => {
    expect(canonKey("filter", "Client Error (4xx)")).toBe("filter-client-error-4xx");
    expect(canonKey("filter", "Over {title.max_chars} Characters")).toBe("filter-over-title-max-chars-characters");
    expect(canonKey("tab", "Response Codes")).toBe("tab-response-codes");
    expect(canonKey("column", "H1-1 Length")).toBe("column-h1-1-length");
  });

  // spec: UI-121
  it("takes the threshold the core resolved from the filter id", () => {
    expect(thresholdOf("Over {title.max_chars} Characters", "Over 60 Characters")).toBe("60");
    expect(thresholdOf("HTML Document Over {validation.html_doc_max_bytes}MB", "HTML Document Over 2MB")).toBe("2");
    expect(thresholdOf("Missing", "Missing")).toBeUndefined();
  });

  // spec: UI-121
  it("shows tabs, filters and columns in German and keeps the canonical name in English", async () => {
    await setLanguage("de");
    const t = getT();
    expect(tabLabel(t, "Response Codes")).toBe("Antwortcodes");
    expect(tabLabel(t, "Internal")).toBe("Intern");
    expect(filterLabel(t, { id: "All", labelTemplate: "All" })).toBe("Alle");
    expect(filterLabel(t, { id: "Over 60 Characters", labelTemplate: "Over {title.max_chars} Characters" })).toBe("Über 60 Zeichen");
    expect(columnLabel(t, "Address")).toBe("Adresse");
    // Unknown names (e.g. from a newer core) fall back to the canonical name, never to a key.
    expect(columnLabel(t, "Brand New Column")).toBe("Brand New Column");
    await setLanguage("en");
    expect(tabLabel(getT(), "Response Codes")).toBe("Response Codes");
    expect(filterLabel(getT(), { id: "Over 60 Characters", labelTemplate: "Over {title.max_chars} Characters" })).toBe("Over 60 Characters");
  });

  // spec: UI-121
  it("translates every canonical key of the catalog into German", () => {
    const keys = ["tab-", "filter-", "column-"];
    const de = (i18n as unknown as { has(lang: string, key: string): boolean });
    for (const key of ["tab-structured-data", "tab-custom-search", "filter-non-indexable", "column-indexability-status"]) {
      expect(keys.some(k => key.startsWith(k))).toBe(true);
      expect(de.has("de", key)).toBe(true);
    }
  });
});

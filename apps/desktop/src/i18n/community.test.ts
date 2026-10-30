import { expect, it } from "vitest";
import { createI18n, loadFtlFiles, i18n } from "./index";
import { languageOptions } from "./languages";

// Outside src: never matched by the production resource glob.
const fixture = import.meta.glob<string>("/e2e/i18n/fixtures/community/i18n/*.ftl", { query: "?raw", import: "default", eager: true });

// spec: UI-121 [P1]
it("discovers a fixture community resource, offers its completion, negotiates system locale and falls back to English", () => {
  const catalog = createI18n(loadFtlFiles(fixture));
  expect(catalog.languages()).toEqual(["en", "pl"]);
  expect(languageOptions(catalog)).toEqual([{ value: "en", label: "English" }, { value: "pl", label: "Polski (50%)" }]);
  expect(catalog.resolve("system", ["pl-PL"])).toBe("pl");
  expect(catalog.resolve("system", ["it-IT"])).toBe("en");
  expect(catalog.translate("pl", "hello", { name: "Ada" })).toBe("Cześć Ada");
  expect(catalog.translate("pl", "button.title")).toBe("Save your changes");
  expect(catalog.translate("pl", "missing")).toBe("English fallback");
  expect(i18n.languages()).not.toContain("pl");
  expect(i18n.has("en", "hello")).toBe(false);
});

// spec: UI-121 [P1]
it("keeps nearly complete community languages visibly incomplete", () => {
  const source = Array.from({ length: 201 }, (_, n) => `key-${n} = Value ${n}`).join("\n");
  const file = (lang: string, source: string) => ({ path: `${lang}.ftl`, lang, feature: "fixture", source });
  const catalog = createI18n([file("en", source), file("pl", source.slice(0, source.lastIndexOf("\n")))]);
  expect(languageOptions(catalog).find(option => option.value === "pl")?.label).toBe("Polski (99%)");
});

// spec: UI-121
it("offers only complete de/en production languages without percentages", () => {
  expect(languageOptions(i18n)).toEqual([
    { value: "en", label: "English" },
    { value: "de", label: "Deutsch" },
  ]);
});

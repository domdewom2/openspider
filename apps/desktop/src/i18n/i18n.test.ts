import { afterEach, describe, expect, it } from "vitest";
import { act, render, screen } from "@testing-library/react";
import { createElement } from "react";
import { checkCatalog } from "./check";
import { createI18n, humanizeKey, parseFtlPath, type FtlFile } from "./core";
import { makeFormatters } from "./format";
import { onLanguageChanged, resolveLanguage, setLanguage, systemLocales, useLanguageStore, useT } from "./index";

const file = (lang: string, source: string, feature = "demo"): FtlFile => ({ path: `/src/${feature}/i18n/${lang}.ftl`, lang, feature, source });

const DE = `
hello = Hallo { $name }
files = { $n ->
    [one] { $n } Datei
   *[other] { $n } Dateien
}
btn = Speichern
    .title = Speichert die Datei
`;
const EN = `
hello = Hello { $name }
files = { $n ->
    [one] { $n } file
   *[other] { $n } files
}
btn = Save
    .title = Saves the file
only-en = Only English
`;

afterEach(async () => {
  await setLanguage("en");
});

describe("i18n-Kern", () => {
  // spec: UI-121
  it("übersetzt mit Argumenten, Plural und Attributen in der gewählten Sprache", () => {
    const i = createI18n([file("de", DE), file("en", EN)]);
    expect(i.translate("de", "hello", { name: "Welt" })).toBe("Hallo Welt");
    expect(i.translate("en", "hello", { name: "World" })).toBe("Hello World");
    expect(i.translate("de", "files", { n: 1 })).toBe("1 Datei");
    expect(i.translate("de", "files", { n: 3 })).toBe("3 Dateien");
    expect(i.translate("de", "btn.title")).toBe("Speichert die Datei");
    expect(i.errors()).toEqual([]);
  });

  // spec: UI-121
  it("fehlende Übersetzung fällt auf Englisch zurück, nie auf den Schlüsselnamen", () => {
    const i = createI18n([file("de", DE), file("en", EN)]);
    expect(i.translate("de", "only-en")).toBe("Only English");
    // weder Sprache noch Englisch: englischer Kanon-Name aus dem Kern
    expect(i.translate("de", "tab-response-codes", undefined, { fallback: "Response Codes" })).toBe("Response Codes");
    // letzter Notnagel: abgeleiteter Name, nicht der Schlüssel
    expect(i.translate("de", "column-meta-description-1")).toBe("Meta Description 1");
  });

  // spec: UI-121
  it("Systemsprache: de-AT/de-CH → Deutsch, nicht unterstützte Sprache → Englisch", () => {
    const i = createI18n([file("de", DE), file("en", EN)]);
    expect(i.resolve("system", ["de-AT"])).toBe("de");
    expect(i.resolve("system", ["de-CH", "en"])).toBe("de");
    expect(i.resolve("system", ["pl-PL"])).toBe("en");
    expect(i.resolve("system", ["pl-PL", "de"])).toBe("de");
    expect(i.resolve("en", ["de-DE"])).toBe("en");
    expect(i.resolve("fr", ["de-DE"])).toBe("de");
  });

  // spec: UI-121
  it("eine zusätzliche Sprachdatei erscheint ohne Codeänderung", () => {
    const i = createI18n([file("de", DE), file("en", EN), file("fr", "btn = Enregistrer")]);
    expect(i.languages()).toEqual(["en", "de", "fr"]);
    expect(i.resolve("system", ["fr-FR"])).toBe("fr");
    expect(i.translate("fr", "hello", { name: "x" })).toBe("Hello x");
  });

  it("parseFtlPath erkennt Feature und Sprache", () => {
    expect(parseFtlPath("/src/features/settings/frame/i18n/de.ftl")).toEqual({ feature: "features/settings/frame", lang: "de" });
    expect(parseFtlPath("/src/ui/i18n/en.ftl")).toEqual({ feature: "ui", lang: "en" });
  });

  it("humanizeKey liefert lesbare Namen", () => {
    expect(humanizeKey("tab-internal")).toBe("Internal");
    expect(humanizeKey("filter-response-codes-client-error")).toBe("Response Codes Client Error");
  });

  // spec: UI-121
  it("Zahlen und Datum folgen der UI-Sprache (de 1.234,5 / en 1,234.5)", () => {
    const de = makeFormatters("de");
    const en = makeFormatters("en");
    expect(de.decimal(1234.5)).toBe("1.234,5");
    expect(en.decimal(1234.5)).toBe("1,234.5");
    expect(de.integer(1234567)).toBe("1.234.567");
    expect(en.bytes(1536000)).toBe("1.54 MB");
    expect(de.bytes(1536000)).toBe("1,54 MB");
    expect(de.percent(0.512, 1)).toMatch(/51,2\s?%/);
    expect(de.duration(3723)).toBe("1:02:03");
    expect(en.duration(125)).toBe("2:05");
    const d = new Date(Date.UTC(2026, 9, 17, 12, 0, 0));
    expect(de.date(d)).not.toBe(en.date(d));
  });
});

describe("Vollständigkeitsprüfung", () => {
  // spec: UI-121
  it("meldet fehlende Schlüssel, abweichende Platzhalter, doppelte Schlüssel und kaputte Einträge", () => {
    const rep = checkCatalog([
      file("de", "a = x { $n }\nb = nur de\nbad = {"),
      file("en", "a = x { $m }\nc = only en"),
      file("en", "c = doppelt", "other"),
      file("de", "c = doppelt", "other"),
    ]);
    expect(rep.problems.join("\n")).toMatch(/Schlüssel b fehlt in en/);
    expect(rep.problems.join("\n")).toMatch(/Schlüssel c fehlt in de/);
    expect(rep.problems.join("\n")).toMatch(/Platzhalter von a weichen ab/);
    expect(rep.problems.join("\n")).toMatch(/ungültiger Eintrag/);
    expect(rep.problems.join("\n")).toMatch(/Schlüssel c \(en\) doppelt/);
  });

  // spec: UI-121
  it("meldet eine fehlende Sprachdatei eines Features und unbekannte Verweise", () => {
    const rep = checkCatalog([file("en", "a = { -brand }\nb = { missing }"), file("de", "a = x", "other")]);
    expect(rep.problems.join("\n")).toMatch(/Sprachdatei fehlt \(de\)/);
    expect(rep.problems.join("\n")).toMatch(/unbekanntes -brand/);
    expect(rep.problems.join("\n")).toMatch(/unbekanntes missing/);
  });

  // spec: UI-121
  it("unvollständige weitere Sprachen sind erlaubt und werden in Prozent gemeldet", () => {
    const rep = checkCatalog([file("de", DE + "only-en = Nur Deutsch\n"), file("en", EN), file("fr", "btn = Enregistrer\nhello = Salut { $name }")]);
    expect(rep.problems).toEqual([]);
    expect(rep.completeness.fr).toBe(40);
    expect(rep.completeness.de).toBe(100);
  });
});

describe("Sprachwechsel zur Laufzeit", () => {
  function Probe() {
    const t = useT();
    return createElement("p", { "data-testid": "probe" }, t("ui-dialog-close"));
  }

  // spec: UI-121
  it("setLanguage rendert wartende Komponenten neu und ruft die Beobachter (Menüneuaufbau) auf", async () => {
    await setLanguage("de");
    render(createElement(Probe));
    expect(screen.getByTestId("probe")).toHaveTextContent("Schließen");
    const seen: string[] = [];
    const off = onLanguageChanged(async (l) => {
      await Promise.resolve();
      seen.push(l);
    });
    await act(async () => {
      await setLanguage("en");
    });
    expect(screen.getByTestId("probe")).toHaveTextContent("Close");
    expect(seen).toEqual(["en"]);
    expect(document.documentElement.lang).toBe("en");
    off();
    await act(async () => {
      await setLanguage("de-unbekannt");
    });
    expect(useLanguageStore.getState().lang).toBe("en");
  });

  it("systemLocales liest navigator.languages", () => {
    expect(systemLocales({ languages: ["de-AT", "en"], language: "de-AT" })).toEqual(["de-AT", "en"]);
    expect(systemLocales({ languages: [], language: "" } as never)).toEqual(["en"]);
    expect(resolveLanguage("system", ["de-AT"])).toBe("de");
  });
});

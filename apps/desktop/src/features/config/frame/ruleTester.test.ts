import { beforeEach, describe, expect, it } from "vitest";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { isTestError, testIncludeExclude, testRewrite } from "./ruleTester";
import { installConfigCore } from "./testing";

let core: MockCore;
beforeEach(() => {
  core = new MockCore();
  installConfigCore(core);
  setClient(createClient(core.transport("main")));
});

const run = async (include: string[], exclude: string[], input: string, partial = false) => {
  const r = await testIncludeExclude({ include, exclude, partial, input });
  if (isTestError(r)) throw new Error(r.error.messageKey);
  return r;
};

describe("Regex-Tester Include/Exclude (CRAWL-111)", () => {
  // spec: CRAWL-111
  it("kodiert die Eingabe wie die Engine, bevor abgeglichen wird (a b passt auf .*%20.*)", async () => {
    const r = await run([".*%20.*"], [], "https://site.test/a b");
    expect(r.encoded).toBe("https://site.test/a%20b");
    expect(r.verdict).toBe("included");
    expect(r.match).toEqual({ list: "include", line: 1 });
    // Der Kern bekommt die kodierte URL, nie die rohe Eingabe.
    const sent = core.calls.filter((c) => c.cmd === "config.testRules").map((c) => (c.args as { input: string }).input);
    expect(sent.every((i) => i === "https://site.test/a%20b")).toBe(true);
  });

  // spec: CRAWL-111
  it("meldet Eingeschlossen, Ausgeschlossen und Nicht eingeschlossen mit der Nummer der passenden Zeile", async () => {
    const inc = await run(["https://site.test/nope/.*", "https://site.test/blog/.*"], [], "https://site.test/blog/a");
    expect([inc.verdict, inc.match]).toEqual(["included", { list: "include", line: 2 }]);
    const exc = await run([], ["https://site.test/x/.*", "https://site.test/private/.*"], "https://site.test/private/a");
    expect([exc.verdict, exc.match]).toEqual(["excluded", { list: "exclude", line: 2 }]);
    const none = await run(["https://site.test/blog/.*"], [], "https://site.test/shop/");
    expect([none.verdict, none.match]).toEqual(["notIncluded", null]);
    const free = await run([], [], "https://site.test/any");
    expect([free.verdict, free.match]).toEqual(["included", null]);
  });

  // spec: CRAWL-111
  it("verwendet die Abgleichsemantik der Engine und weist bei Teilstück-Treffern auf .* hin", async () => {
    const r = await run(["https://site.test/blog"], [], "https://site.test/blog/a");
    expect(r.verdict).toBe("notIncluded");
    expect(r.partialOnly).toEqual([{ list: "include", line: 1 }]);
    const partial = await run(["https://site.test/blog"], [], "https://site.test/blog/a", true);
    expect(partial.verdict).toBe("included");
    expect(partial.partialOnly).toEqual([]);
  });

  // spec: CRAWL-111
  it("markiert Syntaxfehler zeilengenau und testet die übrigen Zeilen trotzdem", async () => {
    const r = await run(["(", "https://site.test/blog/.*"], ["["], "https://site.test/blog/a");
    expect(r.invalid).toEqual(expect.arrayContaining([{ list: "include", line: 1 }, { list: "exclude", line: 1 }]));
    expect(r.invalid).toHaveLength(2);
    expect(r.verdict).toBe("included");
    expect(r.match).toEqual({ list: "include", line: 2 });
  });

  // spec: CRAWL-111
  it("eine ungültige Test-URL wird als Fehler des Kerns gemeldet", async () => {
    const r = await testIncludeExclude({ include: [], exclude: [], partial: false, input: "not a url" });
    expect(isTestError(r)).toBe(true);
  });
});

describe("Testfeld für Umschreibungen (CRAWL-107)", () => {
  // spec: CRAWL-107
  it("zeigt die umgeschriebene URL und das Zwischenergebnis je Schritt", async () => {
    const r = await testRewrite({
      removeParameters: ["utm"],
      removeAllParameters: false,
      rules: [{ pattern: "/old/", replacement: "/new/" }, { pattern: "", replacement: "ignored" }, { pattern: "b=2", replacement: "b=3" }],
      input: "https://site.test/old/x?utm=1&b=2",
    });
    if (isTestError(r)) throw new Error("unexpected");
    expect(r.output).toBe("https://site.test/new/x?b=3");
    expect(r.stages.map((s) => [s.kind, s.line, s.after])).toEqual([
      ["parameters", undefined, "https://site.test/old/x?b=2"],
      ["regex", 1, "https://site.test/new/x?b=2"],
      ["regex", 3, "https://site.test/new/x?b=3"],
    ]);
  });

  // spec: CRAWL-107
  it("rechnet mit der Funktion des Kerns: jeder Aufruf geht über config.testRules, ohne Speichern", async () => {
    await testRewrite({ removeParameters: [], removeAllParameters: true, rules: [], input: "https://site.test/a?x=1" });
    expect(core.calls.map((c) => c.cmd)).toEqual(["config.testRules", "config.testRules"]);
    expect(core.calls.some((c) => c.cmd === "config.set")).toBe(false);
  });

  // spec: CRAWL-107
  it("ungültige Eingaben ergeben einen Fehler statt eines Ergebnisses", async () => {
    const r = await testRewrite({ removeParameters: [], removeAllParameters: false, rules: [], input: "kaputt" });
    expect(isTestError(r)).toBe(true);
  });
});

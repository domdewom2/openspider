import { describe, expect, it } from "vitest";
import type { DiffRow } from "../../../ipc/gen/DiffRow";
import { changedPaths, deviations, getPath, isLiveField, issuesFor, lineIndexOf, lockReason, pageForAnchor, PRESET_KEYS, runModeOf, setPath, STRUCTURAL_FIELDS } from "./model";
import { asSchema, defaultsFromSchema, fieldMeta, nodeAt } from "./schema";
import { SCHEMA, defaultConfig } from "./testing";

const owner = (controllable: boolean) => ({ kind: "app" as const, live: true, controllable });

describe("Pfadzugriff auf die Konfiguration", () => {
  // spec: OUT-220
  it("setPath ändert nur den Pfad und lässt das Original unverändert", () => {
    const base = { speed: { threads: 5, auto: false }, mode: "spider" };
    const next = setPath(base, "speed.threads", 9);
    expect(getPath(next, "speed.threads")).toBe(9);
    expect(getPath(base, "speed.threads")).toBe(5);
    expect(getPath(next, "speed.auto")).toBe(false);
    expect(getPath(setPath({}, "a.b.c", 1), "a.b.c")).toBe(1);
    expect(getPath(base, "speed.missing.deep")).toBeUndefined();
  });

  // spec: OUT-220
  it("changedPaths nennt Blätter, Listen als Ganzes und ignoriert Gleiches", () => {
    const a = { limits: { max_urls: 1, max_depth: null }, scope: { include: ["a"] } };
    const b = { limits: { max_urls: 2, max_depth: null }, scope: { include: ["a", "b"] } };
    expect(changedPaths(a, b)).toEqual(["limits.max_urls", "scope.include"]);
    expect(changedPaths(a, structuredClone(a))).toEqual([]);
  });
});

describe("Sperren nach Crawl-Zustand (F-13/F-14)", () => {
  // spec: CRAWL-126 [M2]
  it("ohne Crawl und im Leerlauf ist alles editierbar", () => {
    expect(runModeOf(null)).toBe("free");
    expect(runModeOf({ phase: "idle", owner: owner(true) })).toBe("free");
    for (const p of ["limits.max_urls", "rendering.mode", "scope.include"]) expect(lockReason(p, "free")).toBeNull();
  });

  // spec: CRAWL-126 [M2]
  it("während „Läuft“ bleiben nur Schwellwerte, Duplikate, Links, Segmente, Analyse und speed.threads editierbar", () => {
    expect(runModeOf({ phase: "running", owner: owner(true) })).toBe("running");
    for (const key of PRESET_KEYS) expect(lockReason(key, "running"), key).toBeNull();
    for (const p of ["duplicates.near_enabled", "links.high_crawl_depth", "segments.items", "analysis.auto_at_end", "speed.threads", "issues.ignore_non_indexable"]) {
      expect(isLiveField(p), p).toBe(true);
      expect(lockReason(p, "running"), p).toBeNull();
    }
    for (const p of ["limits.max_urls", "scope.include", "speed.max_per_host", "robots.mode", "user_agent.http", "http.headers", "auth.basic"]) expect(lockReason(p, "running"), p).toBe("pause");
  });

  // spec: CRAWL-126 [M2]
  it("Modus, Bereich und Rendering-Modus sind weder laufend noch pausiert änderbar („neuer Crawl nötig“)", () => {
    expect(runModeOf({ phase: "paused", owner: owner(true) })).toBe("paused");
    expect(runModeOf({ phase: "completed", owner: owner(true) })).toBe("paused");
    for (const p of STRUCTURAL_FIELDS) {
      expect(lockReason(p, "running"), p).toBe("newCrawl");
      expect(lockReason(p, "paused"), p).toBe("newCrawl");
    }
    for (const p of ["limits.max_urls", "scope.include", "speed.threads", "http.headers", "auth.basic"]) expect(lockReason(p, "paused"), p).toBeNull();
  });

  // spec: CRAWL-126 [M2]
  it("ein fremd gestarteter Crawl macht den Dialog schreibgeschützt", () => {
    expect(runModeOf({ phase: "running", owner: owner(false) })).toBe("foreign");
    expect(runModeOf({ phase: "paused", owner: owner(false) })).toBe("foreign");
    expect(lockReason("speed.threads", "foreign")).toBe("foreign");
  });
});

describe("Abweichungen, Prüfmeldungen, Sprungziele", () => {
  const rows: DiffRow[] = [{ path: "limits.max_urls", labelKey: "x", current: 10, base: 5000000 }];

  // spec: OUT-220
  it("Abweichungen vergleichen den Entwurf mit den Werkseinstellungen aus config.diff", () => {
    const saved = { limits: { max_urls: 10, max_depth: null }, speed: { threads: 5 } };
    const draft = { limits: { max_urls: 5000000, max_depth: null }, speed: { threads: 8 } };
    const found = deviations(draft, saved, rows);
    expect(found.map((d) => [d.path, d.value, d.standard])).toEqual([["speed.threads", 8, 5]]);
    expect(deviations(saved, saved, rows).map((d) => d.path)).toEqual(["limits.max_urls"]);
  });

  // spec: OUT-220
  it("issuesFor ordnet Meldungen eines Listenfeldes zeilenweise zu", () => {
    const issue = (path: string) => ({ path, messageKey: "k", args: {}, line: null, column: null, warning: false });
    const all = [issue("scope.include[2]"), issue("scope.exclude[0]"), issue("limits.max_urls")];
    expect(issuesFor(all, "scope.include").map((i) => i.path)).toEqual(["scope.include[2]"]);
    expect(issuesFor(all, "limits").map((i) => i.path)).toEqual(["limits.max_urls"]);
    expect(lineIndexOf("scope.include[2]")).toBe(2);
    expect(lineIndexOf("limits.max_urls")).toBeNull();
  });

  // spec: OUT-220
  it("Sprungziele: genauer Feldpfad vor Präfix", () => {
    const pages = [
      { id: "speed", anchors: ["speed", "speed.threads"] },
      { id: "spider-rendering", anchors: ["rendering"] },
      { id: "spider-presets", anchors: ["content", "links"] },
    ];
    expect(pageForAnchor(pages, "speed.threads")?.id).toBe("speed");
    expect(pageForAnchor(pages, "rendering.mode")?.id).toBe("spider-rendering");
    expect(pageForAnchor(pages, "rendering")?.id).toBe("spider-rendering");
    expect(pageForAnchor(pages, "content.low_word_count")?.id).toBe("spider-presets");
    expect(pageForAnchor(pages, "nonsense.field")).toBeNull();
  });
});

describe("Schema des Kerns als Quelle für Typen und Grenzen", () => {
  // spec: OUT-220
  it("liest Wertebereiche, Aufzählungen und optionale Werte aus dem JSON-Schema", () => {
    expect(fieldMeta(SCHEMA, "limits.max_urls")).toMatchObject({ type: "integer", nullable: false, minimum: 1 });
    expect(fieldMeta(SCHEMA, "limits.max_depth")).toMatchObject({ type: "integer", nullable: true, minimum: 0 });
    expect(fieldMeta(SCHEMA, "speed.threads")).toMatchObject({ minimum: 1, maximum: 200, default: 5 });
    expect(fieldMeta(SCHEMA, "rendering.mode")?.enum).toEqual(["text", "javascript"]);
    expect(fieldMeta(SCHEMA, "robots.mode")?.enum).toEqual(["respect", "ignore", "ignore_but_report"]);
    expect(fieldMeta(SCHEMA, "scope.include")).toMatchObject({ type: "array", items: "string" });
    expect(fieldMeta(SCHEMA, "extraction.structured_data.json_ld")?.type).toBe("boolean");
    expect(fieldMeta(SCHEMA, "does.not.exist")).toBeNull();
    expect(nodeAt(null, "a")).toBeUndefined();
    expect(asSchema(null)).toBeNull();
  });

  // spec: OUT-220
  it("die aus dem Schema abgeleiteten Werkseinstellungen kennen jeden Seitenpfad", () => {
    const d = defaultsFromSchema(SCHEMA);
    expect(getPath(d, "speed.threads")).toBe(5);
    expect(getPath(d, "limits.max_depth")).toBeNull();
    expect(getPath(d, "rendering.mode")).toBe("text");
    expect(getPath(defaultConfig(), "robots.mode")).toBe("respect");
  });
});

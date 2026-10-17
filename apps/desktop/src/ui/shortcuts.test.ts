import { afterEach, describe, expect, it, vi } from "vitest";
import { installShortcutDispatcher, eventCombo, resolveShortcut, isTextField, type DispatchContext, type KeyLike } from "./dispatcher";
import { registerAction } from "./actions";
import {
  canonicalCombo,
  findDuplicateBindings,
  findReservedCollisions,
  formatShortcut,
  isEditingCombo,
  shortcutById,
  shortcutTable,
  SHORTCUTS,
} from "./shortcuts";

const key = (k: string, mods: Partial<KeyLike> = {}): KeyLike => ({ key: k, ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...mods });
const ctx = (over: Partial<DispatchContext> = {}): DispatchContext => ({ platform: "win", field: false, inTable: false, modal: false, ...over });

describe("Kürzelregister (UI-113)", () => {
  // spec: UI-113
  it("enthält alle Zeilen der Tabelle mit eindeutiger Aktions-ID und Beschriftungsschlüssel", () => {
    const ids = SHORTCUTS.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
    for (const id of [
      "crawl.start",
      "crawl.pause",
      "crawl.new",
      "crawl.open",
      "crawl.saveAs",
      "app.settings",
      "config.spider",
      "config.diff",
      "search.focus",
      "search.clear",
      "table.copy",
      "table.selectAll",
      "export.view",
      "toptab.next",
      "toptab.prev",
      "toptab.goto.1",
      "toptab.goto.9",
      "filter.open",
      "focus.next",
      "focus.prev",
      "view.toggleDetail",
      "view.toggleSidebar",
      "nav.back",
      "nav.forward",
      "browser.open",
      "crawl.recrawl",
      "table.remove",
      "analysis.start",
      "source.nextDiff",
      "source.prevDiff",
      "zoom.in",
      "zoom.out",
      "zoom.reset",
      "context.menu",
    ])
      expect(shortcutById(id), id).toBeDefined();
    expect(SHORTCUTS.every((s) => s.labelKey.startsWith("shortcut-"))).toBe(true);
    // P0-Kern: starten, pausieren, neu, öffnen, Einstellungen, Suchfeld, Suche aufheben, Kopieren, Alles, Exportieren, Kontextmenü
    expect(SHORTCUTS.filter((s) => s.p0).map((s) => s.id).sort()).toEqual(
      ["app.settings", "context.menu", "crawl.new", "crawl.open", "crawl.pause", "crawl.start", "export.view", "search.clear", "search.focus", "table.copy", "table.selectAll"].sort(),
    );
  });

  // spec: UI-113
  it("Plattformbelegung: Cmd auf macOS, Strg auf Windows, Zurück/Vor je Plattform", () => {
    expect(shortcutById("crawl.pause")?.keys).toEqual({ mac: "Cmd+P", win: "Ctrl+P" });
    expect(shortcutById("app.settings")?.keys).toEqual({ mac: "Cmd+,", win: "Ctrl+," });
    expect(shortcutById("nav.back")?.keys).toEqual({ mac: "Cmd+[", win: "Alt+Left" });
    expect(shortcutById("view.toggleSidebar")?.keys.mac).toBe("Cmd+Shift+J");
  });

  // spec: UI-113
  it("keine Kollision mit den reservierten Systemkürzeln und keine doppelte Belegung", () => {
    expect(findReservedCollisions()).toEqual([]);
    expect(findDuplicateBindings()).toEqual([]);
  });

  // spec: UI-113
  it("die Kollisionsprüfung erkennt reservierte Kürzel (Negativtest)", () => {
    const bad = [{ ...(shortcutById("crawl.new") as never as (typeof SHORTCUTS)[number]), id: "x.quit", keys: { mac: "Cmd+Q", win: "Alt+F4" } }];
    expect(findReservedCollisions(bad)).toHaveLength(2);
    const dup = [shortcutById("crawl.new"), { ...shortcutById("crawl.open"), keys: { mac: "Cmd+N", win: "Ctrl+N" } }] as typeof SHORTCUTS;
    expect(findDuplicateBindings(dup)).toHaveLength(2);
  });

  // spec: UI-113
  it("Anzeige: macOS-Symbole in Apple-Reihenfolge, Windows deutsch Strg+Alt+Umschalt, englisch Ctrl+Alt+Shift", () => {
    expect(formatShortcut("Cmd+Alt+Shift+X", "mac", "de")).toBe("⌥⇧⌘X");
    expect(formatShortcut("Ctrl+Alt+Shift+X", "win", "de")).toBe("Strg+Alt+Umschalt+X");
    expect(formatShortcut("Ctrl+Alt+Shift+X", "win", "en")).toBe("Ctrl+Alt+Shift+X");
    expect(formatShortcut("Cmd+Enter", "mac", "en")).toBe("⌘↩");
    expect(formatShortcut("Alt+Left", "win", "de")).toBe("Alt+←");
    expect(formatShortcut("Del", "win", "de")).toBe("Entf");
    expect(formatShortcut("Del", "win", "en")).toBe("Del");
  });

  // spec: UI-113
  it("Hilfe-Tabelle und Register enthalten dieselben Einträge (Anzahl und Belegung)", () => {
    for (const platform of ["mac", "win"] as const) {
      const table = shortcutTable(platform, "en");
      expect(table).toHaveLength(SHORTCUTS.length);
      table.forEach((row, i) => {
        const sc = SHORTCUTS[i];
        expect(row.id).toBe(sc?.id);
        expect(row.keys[0]).toBe(formatShortcut(sc?.keys[platform] as string, platform, "en"));
      });
    }
  });

  it("Bearbeiten-Kürzel sind nie native Accelerators", () => {
    for (const c of ["Cmd+C", "Ctrl+V", "Ctrl+X", "Ctrl+A", "Cmd+Z", "Del", "Backspace", "Alt+Left"]) expect(isEditingCombo(canonicalCombo(c, "mac")), c).toBe(true);
    expect(isEditingCombo(canonicalCombo("Mod+N", "win"))).toBe(false);
  });
});

describe("Dispatcher (Kontextregeln UI-113)", () => {
  // spec: UI-113
  it("löst Kürzel plattformrichtig auf (Cmd auf macOS, Strg auf Windows)", () => {
    expect(resolveShortcut(key("p", { code: "KeyP", ctrlKey: true }), ctx())?.id).toBe("crawl.pause");
    expect(resolveShortcut(key("p", { code: "KeyP", metaKey: true }), ctx({ platform: "mac" }))?.id).toBe("crawl.pause");
    expect(resolveShortcut(key("p", { code: "KeyP", ctrlKey: true }), ctx({ platform: "mac" }))).toBeNull();
    expect(resolveShortcut(key(",", { ctrlKey: true }), ctx())?.id).toBe("app.settings");
    expect(resolveShortcut(key("3", { code: "Digit3", metaKey: true }), ctx({ platform: "mac" }))?.id).toBe("toptab.goto.3");
    expect(resolveShortcut(key("ArrowRight", { ctrlKey: true, altKey: true }), ctx())?.id).toBe("toptab.next");
    expect(resolveShortcut(key("[", { metaKey: true }), ctx({ platform: "mac" }))?.id).toBe("nav.back");
    expect(resolveShortcut(key("ArrowLeft", { altKey: true }), ctx())?.id).toBe("nav.back");
    expect(resolveShortcut(key("L", { code: "KeyL", altKey: true, ctrlKey: true }), ctx())).toBeNull();
  });

  // spec: UI-113
  it("Option/Alt verändert Buchstaben auf macOS nicht (Auflösung über code)", () => {
    expect(eventCombo(key("¬", { code: "KeyL", altKey: true, metaKey: true }), "mac")).toBe("Alt+Cmd+L");
    expect(resolveShortcut(key("Dead", { code: "KeyA", metaKey: true, shiftKey: true }), ctx({ platform: "mac" }))?.id).toBe("analysis.start");
  });

  // spec: UI-113
  it("Zoom: Mod+=, Mod++ und Mod+- und Mod+0", () => {
    expect(resolveShortcut(key("=", { ctrlKey: true }), ctx())?.id).toBe("zoom.in");
    expect(resolveShortcut(key("+", { ctrlKey: true, shiftKey: true }), ctx())?.id).toBe("zoom.in");
    expect(resolveShortcut(key("-", { ctrlKey: true }), ctx())?.id).toBe("zoom.out");
    expect(resolveShortcut(key("0", { code: "Digit0", ctrlKey: true }), ctx())?.id).toBe("zoom.reset");
  });

  // spec: UI-113
  it("Hat ein Texteingabefeld den Fokus, wirken nur Esc, Mod+F und F6/Shift+F6 global; Mod+A/C, Entf und Alt+← gehören dem Feld", () => {
    const field = ctx({ field: true, inTable: true });
    expect(resolveShortcut(key("Escape"), field)?.id).toBe("search.clear");
    expect(resolveShortcut(key("f", { code: "KeyF", ctrlKey: true }), field)?.id).toBe("search.focus");
    expect(resolveShortcut(key("F6"), field)?.id).toBe("focus.next");
    expect(resolveShortcut(key("F6", { shiftKey: true }), field)?.id).toBe("focus.prev");
    for (const k of [
      key("a", { code: "KeyA", ctrlKey: true }),
      key("c", { code: "KeyC", ctrlKey: true }),
      key("Delete"),
      key("Backspace"),
      key("ArrowLeft", { altKey: true }),
      key("n", { code: "KeyN", ctrlKey: true }),
    ])
      expect(resolveShortcut(k, field), k.key).toBeNull();
  });

  // spec: UI-113
  it("Tabellenkürzel wirken nur in der Tabelle; Entf und Backspace entfernen", () => {
    expect(resolveShortcut(key("a", { code: "KeyA", ctrlKey: true }), ctx())).toBeNull();
    expect(resolveShortcut(key("a", { code: "KeyA", ctrlKey: true }), ctx({ inTable: true }))?.id).toBe("table.selectAll");
    expect(resolveShortcut(key("Delete"), ctx({ inTable: true }))?.id).toBe("table.remove");
    expect(resolveShortcut(key("Backspace"), ctx({ inTable: true }))?.id).toBe("table.remove");
  });

  // spec: UI-113
  it("In modalen Dialogen sind globale Kürzel aus", () => {
    expect(resolveShortcut(key("n", { code: "KeyN", ctrlKey: true }), ctx({ modal: true }))).toBeNull();
    expect(resolveShortcut(key("F6"), ctx({ modal: true }))).toBeNull();
  });

  // spec: UI-113
  it("erkennt Textfelder, nicht aber Schaltflächen oder Kontrollkästchen", () => {
    document.body.innerHTML = `<input id="t" type="text"><input id="c" type="checkbox"><textarea id="a"></textarea><button id="b"></button><div id="e" contenteditable="true"></div><div id="r" role="searchbox"></div>`;
    const get = (id: string) => document.getElementById(id);
    expect([...["t", "a", "e", "r"]].every((id) => isTextField(get(id)))).toBe(true);
    expect(["c", "b"].some((id) => isTextField(get(id)))).toBe(false);
  });
});

describe("Dispatcher am Dokument", () => {
  afterEach(() => {
    document.body.innerHTML = "";
  });

  // spec: UI-113
  it("löst die Aktion aus, unterdrückt die Standardaktion und meldet das Aktionsereignis os:action", () => {
    const seen: string[] = [];
    const listener = (e: Event) => seen.push((e as CustomEvent).detail.id);
    document.addEventListener("os:action", listener);
    const handler = vi.fn();
    const off1 = registerAction("crawl.pause", handler);
    const off2 = installShortcutDispatcher({ platform: "win" });
    const ev = new KeyboardEvent("keydown", { key: "p", code: "KeyP", ctrlKey: true, bubbles: true, cancelable: true });
    document.body.dispatchEvent(ev);
    expect(handler).toHaveBeenCalledOnce();
    expect(ev.defaultPrevented).toBe(true);
    expect(seen).toEqual(["crawl.pause"]);
    off2();
    off1();
    document.removeEventListener("os:action", listener);
  });

  // spec: UI-113
  it("ohne Handler bleibt die Standardaktion erhalten; native Accelerators werden nicht doppelt ausgelöst", () => {
    const off = installShortcutDispatcher({ platform: "win", nativeAccelerators: () => new Set(["crawl.new"]) });
    const handler = vi.fn();
    const off2 = registerAction("crawl.new", handler);
    const ev = new KeyboardEvent("keydown", { key: "n", code: "KeyN", ctrlKey: true, bubbles: true, cancelable: true });
    document.body.dispatchEvent(ev);
    expect(handler).not.toHaveBeenCalled();
    const ev2 = new KeyboardEvent("keydown", { key: "k", code: "KeyK", ctrlKey: true, bubbles: true, cancelable: true });
    document.body.dispatchEvent(ev2);
    expect(ev2.defaultPrevented).toBe(false);
    off2();
    off();
  });

  // spec: UI-113
  it("im Suchfeld wirkt Mod+N nicht, Mod+F schon", () => {
    document.body.innerHTML = `<input id="q" type="search">`;
    const calls: string[] = [];
    const off = installShortcutDispatcher({ platform: "win", onAction: (id) => void calls.push(id) });
    const input = document.getElementById("q") as HTMLElement;
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "n", code: "KeyN", ctrlKey: true, bubbles: true }));
    input.dispatchEvent(new KeyboardEvent("keydown", { key: "f", code: "KeyF", ctrlKey: true, bubbles: true }));
    expect(calls).toEqual(["search.focus"]);
    off();
  });
});

import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { setLanguage } from "../i18n";
import { createClient, setClient } from "../ipc/client";
import { createEventBus } from "../ipc/events";
import type { Tick } from "../ipc/gen/Tick";
import { MockCore } from "../ipc/transport/mock";
import { DEFAULT_APPEARANCE } from "../ui/tokens/theme";
import { normalizeSettings, usePrefs } from "../state/prefs";
import { attachSession, useSession } from "../state/session";
import tickEvent from "../ipc/__fixtures__/UiEvent.tick.json";
import settingsFixture from "../ipc/__fixtures__/AppSettingsDto.json";

const baseTick = (tickEvent as { tick: Tick }).tick;

beforeEach(() => {
  useSession.setState({ seq: -1, notices: [], views: {}, ops: {} });
  usePrefs.setState({ loaded: false, dto: null, error: null });
});
afterEach(async () => {
  setClient(null);
  await setLanguage("en");
});

describe("Prefs-Store", () => {
  // spec: UI-120
  it("normalisiert die Antwort des Kerns; Unlesbares fällt auf Standardwerte", () => {
    const n = normalizeSettings(settingsFixture as never);
    expect(n.appearance).toEqual(DEFAULT_APPEARANCE);
    expect(n.languageSetting).toBe("system");
    const bad = normalizeSettings({
      general: { language: "", notifications: true },
      appearance: { theme: "neon", accent: "grün", zoomPct: 137, rowDensity: "huge", zebra: "ja", liveIntervalMs: 50 },
    } as never);
    expect(bad.appearance).toEqual(DEFAULT_APPEARANCE);
    expect(bad.liveIntervalMs).toBe(250);
    expect(normalizeSettings({ general: {}, appearance: { liveIntervalMs: 99999 } } as never).liveIntervalMs).toBe(5000);
  });

  // spec: UI-120
  it("lädt settings.get; patch wirkt sofort und wird mit der Antwort des Kerns abgeglichen", async () => {
    const core = new MockCore();
    setClient(createClient(core.transport()));
    await usePrefs.getState().load();
    expect(usePrefs.getState().appearance.theme).toBe("system");
    const p = usePrefs.getState().patch({ appearance: { theme: "dark", zoomPct: 125 } });
    expect(usePrefs.getState().appearance.theme).toBe("dark"); // optimistisch, vor der Antwort
    await p;
    expect(core.settings.appearance.theme).toBe("dark");
    expect(usePrefs.getState().appearance.zoomPct).toBe(125);
  });

  // spec: UI-120
  it("ein abgelehnter Patch stellt den alten Zustand wieder her und meldet den Fehler", async () => {
    const core = new MockCore();
    core.handlers.set("settings.set", () => {
      throw Object.assign(new Error("x"), { code: "InvalidInput", messageKey: "err-invalid", args: {}, retryable: false });
    });
    setClient(createClient(core.transport()));
    await usePrefs.getState().load();
    await usePrefs.getState().patch({ appearance: { theme: "dark" } });
    expect(usePrefs.getState().appearance.theme).toBe("system");
    expect(usePrefs.getState().error).not.toBeNull();
  });

  // spec: UI-121
  it("Sprache: System löst gegen die Systemsprache auf, Sprachname gilt direkt; die Einstellung bleibt im Kern", async () => {
    const core = new MockCore();
    setClient(createClient(core.transport()));
    await usePrefs.getState().load();
    await usePrefs.getState().patch({ general: { language: "de" } });
    expect(usePrefs.getState().language).toBe("de");
    expect(core.settings.general.language).toBe("de");
    await usePrefs.getState().patch({ general: { language: "en" } });
    expect(usePrefs.getState().language).toBe("en");
  });

  // spec: UI-120
  it("resetAppearance setzt Theme, Akzent, Zoom, Dichte und Zebra auf die Standardwerte", async () => {
    const core = new MockCore();
    setClient(createClient(core.transport()));
    await usePrefs.getState().load();
    await usePrefs.getState().patch({ appearance: { theme: "dark", accent: "#3c79ce", zoomPct: 150, rowDensity: "normal", zebra: false } });
    await usePrefs.getState().resetAppearance();
    expect(usePrefs.getState().appearance).toEqual(DEFAULT_APPEARANCE);
  });
});

describe("Sitzungs-Store", () => {
  it("wird ausschließlich durch Ticks geschrieben; ältere Ticks werden verworfen", () => {
    const bus = createEventBus();
    const off = attachSession(bus);
    bus.emit({ kind: "tick", tick: { ...baseTick, seq: 5, countsVersion: 3, notices: [{ level: "info", code: "crawl_done", args: {} }] } });
    bus.emit({ kind: "tick", tick: { ...baseTick, seq: 4, countsVersion: 9 } });
    const s = useSession.getState();
    expect(s.seq).toBe(5);
    expect(s.countsVersion).toBe(3);
    expect(s.notices).toHaveLength(1);
    bus.emit({ kind: "tick", tick: { ...baseTick, seq: 6, views: [{ viewId: "v1", version: 2, total: 10 }], ops: [{ opId: "o1", labelKey: "x", done: 1, state: "running" } as never] } });
    expect(useSession.getState().views.v1?.version).toBe(2);
    expect(useSession.getState().ops.o1).toBeDefined();
    useSession.getState().clearNotices();
    expect(useSession.getState().notices).toEqual([]);
    off();
  });
});

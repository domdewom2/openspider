import { ACCENT_PALETTE } from "../../../ui/tokens/tokens";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { startAppearanceSync, type AppearanceHandle } from "../../../app/appearance";
import { setLanguage } from "../../../i18n";
import { createClient, setClient } from "../../../ipc/client";
import { MockCore } from "../../../ipc/transport/mock";
import { usePrefs } from "../../../state/prefs";
import { DEFAULT_APPEARANCE } from "../../../ui/tokens/theme";
import { SettingsFrame, focusSettingAnchor } from "./SettingsFrame";
import { resolveSections, sectionOfAnchor } from "./registry";
import { KNOWN_SECTIONS } from "./types";

let core: MockCore;
let appearance: AppearanceHandle;
let zoomCalls: number[];

beforeEach(async () => {
  core = new MockCore();
  setClient(createClient(core.transport("settings")));
  usePrefs.setState({ loaded: false, dto: null, appearance: DEFAULT_APPEARANCE, language: "en", languageSetting: "system" });
  await usePrefs.getState().load();
  zoomCalls = [];
  document.documentElement.removeAttribute("data-theme");
  appearance = startAppearanceSync({ platform: "win", setZoom: (f) => void zoomCalls.push(f) });
});
afterEach(async () => {
  appearance.stop();
  cleanup();
  setClient(null);
  await act(async () => {
    await setLanguage("en");
  });
});

describe("Einstellungsfenster: Rahmen", () => {
  it("kennt alle Abschnitte aus 1.5; Allgemein und Oberfläche sind fertig, die übrigen Platzhalter", () => {
    const sections = resolveSections();
    expect(sections.map((s) => s.id)).toEqual(KNOWN_SECTIONS.map((k) => k.id));
    expect(sections.filter((s) => s.Component).map((s) => s.id)).toEqual(["general", "appearance", "storage", "crawls", "exports", "network", "accounts", "email", "mcp"]);
    expect(sectionOfAnchor("appearance.zoom")).toBe("appearance");
  });

  it("Seitennavigation mit Platzhalter für noch nicht gelieferte Abschnitte", () => {
    render(<SettingsFrame />);
    const nav = screen.getByRole("tablist", { name: "Settings sections" });
    expect(within(nav).getAllByRole("tab")).toHaveLength(KNOWN_SECTIONS.length);
    fireEvent.click(within(nav).getByRole("tab", { name: "Storage" }));
    expect(document.querySelector('[data-settings-section="storage"]')).not.toBeNull();
  });

  // spec: UI-016
  it("ein Anker öffnet den Abschnitt und setzt den Fokus auf das Ziel-Steuerelement", async () => {
    render(<SettingsFrame anchor="appearance.zoom" />);
    await waitFor(() => expect(document.querySelector('[data-settings-section="appearance"]')).not.toBeNull());
    expect(focusSettingAnchor("appearance.zoom")).toBe(true);
    expect((document.activeElement as HTMLElement).closest('[data-setting-anchor="appearance.zoom"]')).not.toBeNull();
    expect(focusSettingAnchor("gibt.es.nicht")).toBe(false);
  });
});

describe("Einstellungen: Oberfläche (UI-120)", () => {
  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("Theme-Wechsel wirkt sofort auf das Dokument und wird im Kern gespeichert", async () => {
    render(<SettingsFrame anchor="appearance" />);
    const group = await screen.findByRole("radiogroup", { name: "Theme" });
    expect(document.documentElement.dataset.theme).toBe("light"); // Standard „System“, OS hell (jsdom)
    fireEvent.click(within(group).getByRole("radio", { name: "Dark" }));
    await waitFor(() => expect(document.documentElement.dataset.theme).toBe("dark"));
    expect(document.documentElement.style.getPropertyValue("--os-bg-surface").toUpperCase()).toBe("#1B1D20");
    await waitFor(() => expect(core.settings.appearance.theme).toBe("dark"));
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("Dichte kompakt (28 px) ist Standard; normal setzt 44 px", async () => {
    render(<SettingsFrame anchor="appearance" />);
    const group = await screen.findByRole("radiogroup", { name: "Row height in tables" });
    expect(group).toBeInTheDocument();
    expect(document.documentElement.style.getPropertyValue("--os-row-height")).toBe("28px");
    fireEvent.click(within(group).getByRole("radio", { name: /Comfortable/ }));
    await waitFor(() => expect(document.documentElement.style.getPropertyValue("--os-row-height")).toBe("44px"));
    expect(document.documentElement.dataset.density).toBe("normal");
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("Akzent #FFFF66 zeigt die Kontrastwarnung und setzt die Rahmen-Markierung; Speichern bleibt möglich", async () => {
    render(<SettingsFrame anchor="appearance" />);
    const group = await screen.findByRole("group", { name: "Accent color" });
    fireEvent.change(within(group).getByRole("textbox"), { target: { value: "#FFFF66" } });
    await waitFor(() => expect(screen.getByTestId("accent-warning")).toBeInTheDocument());
    expect(document.documentElement.dataset.accentWarn).toBe("on");
    await waitFor(() => expect(core.settings.appearance.accent).toBe("#ffff66"));
  });

  // K2-63: Dichte-Erwartungen nach K2-77 (28/44 px).
  // spec: UI-120
  it("Zoom wird über den WebView-Zoom gesetzt; Zurücksetzen stellt die Standardwerte wieder her", async () => {
    render(<SettingsFrame anchor="appearance" />);
    await screen.findByRole("radiogroup", { name: "Theme" });
    await act(async () => {
      await usePrefs.getState().patch({ appearance: { zoomPct: 125, theme: "dark", zebra: false } });
    });
    expect(zoomCalls.at(-1)).toBe(1.25);
    expect(document.documentElement.dataset.zebra).toBe("off");
    fireEvent.click(screen.getByRole("button", { name: "Reset appearance" }));
    await waitFor(() => expect(usePrefs.getState().appearance).toEqual(DEFAULT_APPEARANCE));
    expect(zoomCalls.at(-1)).toBe(1);
    expect(document.documentElement.dataset.theme).toBe("light");
  });

  it("Live-Intervall außerhalb 250 – 5000 ms zeigt einen Fehler und wird nicht gespeichert", async () => {
    render(<SettingsFrame anchor="appearance" />);
    const input = await screen.findByLabelText("Live update interval (ms)");
    fireEvent.change(input, { target: { value: "100" } });
    expect(await screen.findByText(/Enter a whole number between 250 and 5,000/)).toBeInTheDocument();
    expect(core.settings.appearance.liveIntervalMs).toBe(500);
    fireEvent.change(input, { target: { value: "1000" } });
    await waitFor(() => expect(core.settings.appearance.liveIntervalMs).toBe(1000));
  });
});

describe("Einstellungen: Sprache (UI-121)", () => {
  // spec: UI-121
  it("Sprachwechsel wirkt ohne Neustart auf das Einstellungsfenster und bleibt in den Einstellungen gespeichert", async () => {
    render(<SettingsFrame anchor="general" />);
    const select = await screen.findByRole("button", { name: /Language/ });
    expect(select).toBeInTheDocument();
    await act(async () => {
      await usePrefs.getState().patch({ general: { language: "de" } });
    });
    expect(await screen.findByRole("tablist", { name: "Einstellungsbereiche" })).toBeInTheDocument();
    expect(screen.getByText("Benachrichtigungen anzeigen")).toBeInTheDocument();
    expect(document.documentElement.lang).toBe("de");
    expect(core.settings.general.language).toBe("de");
  });

  // spec: OUT-271
  it("Sprache und Theme stehen in den App-Einstellungen (settings.toml) und überleben ein neues Laden", async () => {
    await usePrefs.getState().patch({ general: { language: "de" }, appearance: { theme: "dark" } });
    usePrefs.setState({ dto: null, loaded: false, language: "en" });
    await usePrefs.getState().load();
    expect(usePrefs.getState().language).toBe("de");
    expect(usePrefs.getState().appearance.theme).toBe("dark");
  });

  // spec: CRAWL-137
  it("der Sprachwechsel berührt nur die App-Einstellungen: keine Crawl-, Export- oder Konfigurationsbefehle (laufende Crawls bleiben unbeeinflusst)", async () => {
    const before = JSON.stringify({ ...core.settings, general: undefined });
    core.calls.length = 0;
    await usePrefs.getState().patch({ general: { language: "de" } });
    await setLanguage("de");
    expect(JSON.stringify({ ...core.settings, general: undefined })).toBe(before);
    const commands = core.calls.map((c) => c.cmd);
    expect(commands).toEqual(["settings.set"]);
    expect(commands.some((c) => /^(crawl|export|config|profile|crawls)\./.test(c))).toBe(false);
  });

  // spec: UI-121
  it("ein Sprachwechsel in einem Fenster erreicht das zweite Fenster über settingsChanged", async () => {
    const mainClient = createClient(core.transport("main"));
    const seen: string[][] = [];
    mainClient.subscribe("main", (e) => e.kind === "settingsChanged" && seen.push(e.keys));
    await usePrefs.getState().patch({ general: { language: "de" } });
    expect(seen).toEqual([["general.language"]]);
  });
});

// spec: UI-120
it("K2-77 displays the legacy default as Blue and still allows the Green preset without migrating settings", async () => {
  render(<SettingsFrame anchor="appearance" />);
  const blue = await screen.findByRole("button", { name: "Blue" });
  expect(blue).toHaveAttribute("aria-pressed", "true");
  expect(core.settings.appearance.accent.toLowerCase()).toBe("#2f855a");
  const green = ACCENT_PALETTE.find(c => c.id === "green")!;
  fireEvent.click(screen.getByRole("button", { name: "Green" }));
  await waitFor(() => expect(core.settings.appearance.accent).toBe(green.hex.toLowerCase()));
  await waitFor(() => expect(document.documentElement.style.getPropertyValue("--os-accent")).toBe(green.hex.toUpperCase()));
});

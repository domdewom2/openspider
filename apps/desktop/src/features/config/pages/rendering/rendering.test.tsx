// A02-19 and A02-21 (M2-U34): notes and controls of the configuration pages Segments and Spider › Rendering.
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { setLanguage } from "../../../../i18n";
import { createClient, setClient } from "../../../../ipc/client";
import { MockCore } from "../../../../ipc/transport/mock";
import type { CrawlStatus } from "../../../../ipc/gen/CrawlStatus";
import { useSession } from "../../../../state/session";
import { ConfigHost } from "../../frame/ConfigHost";
import { closeConfig, openConfig } from "../../frame/store";
import { installConfigCore, type FakeConfigCore } from "../../frame/testing";
import fixture from "../custom-search/fixtures/fach.json";

let core: MockCore; let fake: FakeConfigCore;
async function open(anchor: string) { await act(async () => openConfig({ anchor })); return screen.findByRole("dialog", { name: /^(Crawl configuration|Crawl-Konfiguration)$/ }); }
beforeEach(async () => {
  core = new MockCore(); fake = installConfigCore(core); fake.config = { ...fake.config, ...structuredClone(fixture) };
  core.handlers.set("compare.contentAvailable", () => false);
  core.handlers.set("api.list", () => []);
  core.handlers.set("catalog.columns", () => []);
  setClient(createClient(core.transport("main"))); await setLanguage("en");
  useSession.setState({ crawl: null, ops: {}, analysis: null });
});
afterEach(async () => { act(() => closeConfig()); cleanup(); await setLanguage("en"); });

describe("Spider › Rendering explains why JavaScript rendering cannot be chosen (A02-21)", () => {
  // spec: UI-016
  it("names the new crawl and the Chromium requirement while the mode is locked for the open crawl", async () => {
    useSession.setState({ crawl: { crawlId: "c1", phase: "completed", owner: { controllable: true } } as unknown as CrawlStatus });
    render(<ConfigHost />);
    const dialog = await open("rendering.mode");
    expect(await within(dialog).findByTestId("rendering-locked-note")).toHaveTextContent("only be changed in a new crawl");
    expect(within(dialog).getByTestId("rendering-locked-how")).toHaveTextContent("“Clear” in the header and start a new crawl");
    expect(within(dialog).getByTestId("rendering-chromium-note")).toHaveTextContent("JavaScript rendering needs Chromium");
    expect(within(dialog).getByTestId("rendering-chromium-note")).toHaveTextContent("does not ship its own Chromium yet");
  });

  // spec: UI-016
  it("without an open crawl the mode is free; the Chromium note still says what JavaScript rendering needs", async () => {
    render(<ConfigHost />);
    const dialog = await open("rendering.mode");
    expect(await within(dialog).findByTestId("rendering-chromium-note")).toBeInTheDocument();
    expect(within(dialog).queryByTestId("rendering-locked-how")).toBeNull();
  });
});

describe("Segments: JSON field and file import (A02-19)", () => {
  // spec: UI-068
  it("shows the JSON field below its label and a translated file choice instead of the browser's file input", async () => {
    await setLanguage("de");
    render(<ConfigHost />);
    const dialog = await open("segments");
    const json = await within(dialog).findByRole("textbox", { name: "Segmentliste als JSON" });
    const field = json.closest(".os-segments-json")!;
    expect(field.firstElementChild).toHaveTextContent("Segmentliste als JSON");
    expect(field.firstElementChild?.tagName).toBe("LABEL");
    const file = within(dialog).getByRole("group", { name: "Datei importieren" });
    expect(within(file).getByRole("button", { name: "Datei wählen…" })).toBeInTheDocument();
    expect(within(file).getByRole("status")).toHaveTextContent("Keine Datei ausgewählt");
    const input = file.querySelector<HTMLInputElement>("input[type=file]")!;
    expect(input.hidden).toBe(true);
    const text = JSON.stringify([{ name: "Blog", color: "#1F62C4", groups: [{ conditions: [{ field: "Address", op: "contains", value: "/blog/" }] }] }]);
    const upload = new File([text], "segmente.json", { type: "application/json" });
    Object.defineProperty(upload, "text", { value: () => Promise.resolve(text) });
    await act(async () => { fireEvent.change(input, { target: { files: [upload] } }); });
    expect(within(file).getByRole("status")).toHaveTextContent("segmente.json");
    expect(within(dialog).getByRole("group", { name: "Blog" })).toBeInTheDocument();
  });
});

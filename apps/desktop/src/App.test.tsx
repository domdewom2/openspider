import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { act, cleanup, render, screen } from "@testing-library/react";
import { App } from "./App";
import { setLanguage } from "./i18n";
import { createClient, setClient } from "./ipc/client";
import { MockCore } from "./ipc/transport/mock";

beforeEach(() => {
  setClient(createClient(new MockCore().transport()));
});
afterEach(async () => {
  cleanup();
  setClient(null);
  await act(async () => {
    await setLanguage("en");
  });
});

describe("App (Hauptfenster-Gerüst)", () => {
  it("zeigt die Landmarks der Informationsarchitektur und die Version aus app.info", async () => {
    render(<App />);
    expect(screen.getByRole("banner")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "OpenSpider" })).toBeInTheDocument();
    expect(screen.getByRole("navigation", { name: "Data views" })).toBeInTheDocument();
    expect(screen.getByRole("main")).toBeInTheDocument();
    expect(screen.getByRole("complementary", { name: "Sidebar" })).toBeInTheDocument();
    expect(screen.getByRole("contentinfo")).toBeInTheDocument();
    expect(await screen.findByTestId("version")).toHaveTextContent("Version 0.0.0-mock");
  });

  // spec: UI-016
  it("die leere Tabelle trägt einen Leerzustand mit data-empty-state", () => {
    const { container } = render(<App />);
    expect(container.querySelector('[data-empty-state="NO_DATA_YET"]')).not.toBeNull();
  });

  // spec: UI-121
  it("Sprachwechsel wirkt sofort auf Beschriftungen des Hauptfensters", async () => {
    render(<App />);
    expect(screen.getByRole("navigation", { name: "Data views" })).toBeInTheDocument();
    await act(async () => {
      await setLanguage("de");
    });
    expect(screen.getByRole("navigation", { name: "Datenansichten" })).toBeInTheDocument();
    expect(await screen.findByTestId("version")).toHaveTextContent("Version 0.0.0-mock");
  });
});

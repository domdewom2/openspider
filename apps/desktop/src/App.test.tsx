import { afterEach, describe, expect, it, vi } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "app_version") return "1.2.3";
    throw new Error(`unbekannter Befehl: ${cmd}`);
  }),
}));

import { App } from "./App";

afterEach(cleanup);

describe("App", () => {
  it("zeigt den Titel und die Version aus dem Tauri-Command an", async () => {
    render(<App />);
    expect(screen.getByRole("heading", { name: "OpenSpider" })).toBeInTheDocument();
    expect(await screen.findByTestId("version")).toHaveTextContent("Version 1.2.3");
  });
});

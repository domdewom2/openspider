import { afterEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { Header } from "./Header";
import { setupHeader, tick } from "./fixtures/setup";
import { useSession } from "../../state/session";
import { crawl } from "../../ipc/crawl";
import { setLanguage } from "../../i18n";

afterEach(() => { cleanup(); vi.restoreAllMocks(); });
// spec: UI-003
// spec: CRAWL-126 [M2]
it.each(["pause", "stop"] as const)("locks %s immediately, until a core status acknowledges completion", async action => {
  setupHeader(); await setLanguage("en");
  useSession.getState().applyTick(tick);
  let resolve!: () => void;
  const command = vi.spyOn(crawl, action).mockImplementation(() => new Promise<void>(r => { resolve = r; }));
  render(<Header />);
  fireEvent.click(screen.getByRole("button", { name: action === "pause" ? "Pause" : "Stop" }));
  const pending = screen.getByRole("button", { name: action === "pause" ? "Pausing …" : "Stopping …" });
  expect(pending).toBeDisabled();
  fireEvent.click(pending);
  expect(command).toHaveBeenCalledTimes(1);
  await act(async () => { resolve(); });
  expect(pending).toBeDisabled();
  act(() => useSession.getState().applyTick({ ...tick, seq: 2, crawl: { ...tick.crawl!, phase: action === "pause" ? "paused" : "completed", endReason: action === "stop" ? "stopped" : null } }));
  if (action === "pause") expect(await screen.findByRole("button", { name: "Resume" })).toBeEnabled();
  else {
    expect(screen.queryByRole("button", { name: "Resume" })).toBeNull();
    expect(screen.getByRole("button", { name: "Stop" })).toBeDisabled();
  }
});

// spec: UI-003
it.each(["pause", "stop"] as const)("unlocks when natural completion wins the race with %s", async action => {
  setupHeader(); await setLanguage("en");
  useSession.getState().applyTick(tick);
  const command = vi.spyOn(crawl, action).mockResolvedValue(undefined);
  render(<Header />);
  fireEvent.click(screen.getByRole("button", { name: action === "pause" ? "Pause" : "Stop" }));
  await act(async () => { await Promise.resolve(); });
  expect(command).toHaveBeenCalledTimes(1);
  act(() => useSession.getState().applyTick({ ...tick, seq: 2, crawl: { ...tick.crawl!, phase: "completed", endReason: "queue_empty" } }));
  expect(screen.getByRole("button", { name: "Clear" })).toBeEnabled();
});

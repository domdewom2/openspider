import { afterEach, beforeEach, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { Header } from "./Header";
import { useHeader, normalizeInput, rememberUrl } from "./model";
import { setupHeader, tick } from "./fixtures/setup";
import { useSession } from "../../state/session";
import { setClient } from "../../ipc/client";
import { eventBus } from "../../ipc/events";
import { setLanguage } from "../../i18n";
import { runAction } from "../../ui/actions";

let core: ReturnType<typeof setupHeader>;
beforeEach(async () => { core = setupHeader(); await setLanguage("en"); });
afterEach(() => { cleanup(); setClient(null); });
const start = () => screen.getByRole("button", { name: "Start" });
const calls = (cmd: string) => core.calls.filter((c) => c.cmd === cmd);
const enterUrl = (value: string) => fireEvent.change(screen.getByLabelText("Start URL"), { target: { value } });

// spec: UI-003
it("validates fixture inputs, normalizes HTTPS, and submits Enter through the core validator", async () => {
  render(<Header />);
  expect(start()).toBeDisabled();
  for (const value of ["javascript:alert(1)", "ht tp://x", "https://ex ample.test", "ftp://site.test", "htp:/x"]) {
    enterUrl(value); expect(start()).toBeDisabled();
    expect(screen.getByText("Enter a valid HTTP or HTTPS address.")).toBeVisible();
  }
  enterUrl("  127.0.0.1:8123/sub/  ");
  fireEvent.submit(start().closest("form")!);
  await waitFor(() => expect(calls("crawl.start")).toHaveLength(1));
  expect(calls("crawl.validateStartUrl")[0]?.args).toEqual({ input: "https://127.0.0.1:8123/sub/" });
  expect(calls("crawl.start")[0]?.args).toEqual({ source: { kind: "spider", url: "https://127.0.0.1:8123/sub/" }, scope: "domain" });
  expect(normalizeInput("site.test/a b")).toBe("https://site.test/a%20b");
});

// spec: CRAWL-001 [M2]
it("keeps the newest 20 unique suggestions across remounts, preserving other UI state", async () => {
  core.uiState.state = { table: { width: 99 }, header: { history: ["https://old.test/", "https://old.test/"] } };
  const view = render(<Header />);
  await waitFor(() => expect(document.querySelectorAll("datalist option")).toHaveLength(1));
  enterUrl("site.test"); fireEvent.click(start());
  await waitFor(() => expect(calls("uistate.save")).toHaveLength(1));
  expect(core.uiState.state).toMatchObject({ table: { width: 99 }, header: { history: ["https://site.test/", "https://old.test/"] } });
  view.unmount(); render(<Header />);
  await waitFor(() => expect([...document.querySelectorAll("datalist option")].map((n) => n.getAttribute("value"))).toEqual(["https://site.test/", "https://old.test/"]));
  const many = Array.from({ length: 25 }, (_, n) => `https://site.test/${n}`);
  expect(rememberUrl(many, many[4]!)).toEqual([many[4], ...many.filter((u) => u !== many[4])].slice(0, 20));
});

// spec: CRAWL-001 [M2]
it("requires pause before starting another crawl and mirrors state only from ticks", async () => {
  useSession.getState().applyTick(tick);
  render(<Header />); enterUrl("next.test");
  // K2-63/K2-77: one primary control changes from Start to Pause.
  expect(screen.queryByRole("button", { name: "Start" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Pause" }));
  await waitFor(() => expect(calls("crawl.pause")).toHaveLength(1));
  expect(useSession.getState().crawl?.phase).toBe("running");
  act(() => useSession.getState().applyTick({ ...tick, seq: 2, crawl: { ...tick.crawl!, phase: "paused" } }));
  expect(await screen.findByRole("button", { name: "Resume" })).toBeEnabled();
  expect(screen.queryByRole("button", { name: "Start" })).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "Clear" }));
  await waitFor(() => expect(calls("crawl.clear")).toHaveLength(1));
  expect(screen.queryByRole("dialog")).toBeNull();
});

// spec: UI-003
it("resumes an interrupted or restored paused crawl without requiring a URL", async () => {
  useSession.getState().applyTick({ ...tick, crawl: { ...tick.crawl!, phase: "incomplete" } });
  render(<Header />);
  // K2-63/K2-77: interrupted crawl offers Resume as its primary action.
  expect(screen.queryByRole("button", { name: "Start" })).toBeNull();
  act(() => { runAction("crawl.pause"); });
  await waitFor(() => expect(calls("crawl.resume")).toHaveLength(1));
});

// spec: UI-003
it("asks before recrawling completed data and serializes fast repeated Start actions", async () => {
  useSession.getState().applyTick({ ...tick, crawl: { ...tick.crawl!, phase: "completed" } });
  render(<Header />); enterUrl("next.test");
  fireEvent.click(start()); fireEvent.click(start());
  expect(await screen.findByRole("dialog")).toBeVisible();
  expect(calls("crawl.start")).toHaveLength(0);
  fireEvent.click(screen.getByRole("button", { name: "Replace" }));
  await waitFor(() => expect(calls("crawl.start")).toHaveLength(1));
  expect(calls("crawl.validateStartUrl")).toHaveLength(1);
});

// spec: CRAWL-009 [M2]
it("warns about a missing slash only for folder paths without a filename", () => {
  render(<Header />); enterUrl("https://site.test/blog");
  fireEvent.change(screen.getByLabelText("Scope"), { target: { value: "folder" } });
  expect(screen.getByText(/Without a trailing slash/)).toBeVisible();
  enterUrl("https://site.test/blog/index.html"); expect(screen.queryByText(/Without a trailing slash/)).toBeNull();
  enterUrl("https://site.test/blog/"); expect(screen.queryByText(/Without a trailing slash/)).toBeNull();
});

// spec: CRAWL-026 [M2]
it("confirms a mode change, clears only the view, and retains configuration", async () => {
  useSession.getState().applyTick({ ...tick, crawl: { ...tick.crawl!, phase: "paused" } });
  render(<Header />);
  fireEvent.change(screen.getByLabelText("Mode"), { target: { value: "list" } });
  fireEvent.click(await screen.findByRole("button", { name: "Cancel" }));
  expect(useHeader.getState().mode).toBe("spider");
  fireEvent.change(screen.getByLabelText("Mode"), { target: { value: "list" } });
  fireEvent.click(await screen.findByRole("button", { name: "Replace" }));
  await waitFor(() => expect(useHeader.getState().mode).toBe("list"));
  expect(calls("crawl.clear")).toHaveLength(1);
  expect(calls("config.set")).toHaveLength(0);
  expect(screen.queryByLabelText("Scope")).toBeNull();
});

// spec: CRAWL-112 [M2]
it("warns for configured concurrency from 20 threads, including live changes", async () => {
  render(<Header />);
  expect(screen.queryByText(/20 or more threads/)).toBeNull();
  core.handlers.set("config.get", () => ({ config: { speed: { threads: 20 } } }));
  act(() => eventBus.emit({ kind: "settingsChanged", keys: ["speed.threads"] }));
  expect(await screen.findByText(/20 or more threads/)).toBeVisible();
});

// spec: UI-003
it("imports a complete URL list without truncating to the core preview sample", async () => {
  render(<Header />);
  fireEvent.change(screen.getByLabelText("Mode"), { target: { value: "list" } });
  const items = [...Array.from({ length: 40 }, (_, i) => `https://site.test/${i}`), "https://site.test/0"];
  fireEvent.change(screen.getByLabelText("Paste URLs (one per line)"), { target: { value: items.join("\n") } });
  fireEvent.click(start());
  await waitFor(() => expect(calls("crawl.start")).toHaveLength(1));
  expect(calls("crawl.start")[0]?.args).toMatchObject({ source: { kind: "list", items } });
});

// spec: UI-003
it("disables control commands for a foreign live crawl", () => {
  useSession.getState().applyTick({ ...tick, crawl: { ...tick.crawl!, owner: { ...tick.crawl!.owner, controllable: false } } });
  render(<Header />); enterUrl("next.test");
  expect(screen.queryByRole("button", { name: "Start" })).toBeNull(); expect(screen.getByRole("button", { name: "Pause" })).toBeDisabled();
  expect(screen.getByRole("button", { name: "Clear" })).toBeDisabled();
  act(() => { runAction("crawl.pause"); runAction("crawl.new"); });
  expect(calls("crawl.pause")).toHaveLength(0); expect(calls("crawl.clear")).toHaveLength(0);
});

// spec: UI-003
it("passes sitemap URLs to the existing engine list planner without fetching in the UI", async () => {
  render(<Header />);
  fireEvent.change(screen.getByLabelText("Mode"), { target: { value: "list" } });
  fireEvent.change(screen.getByLabelText("Source"), { target: { value: "sitemap" } });
  expect(start()).toBeDisabled();
  fireEvent.change(screen.getByLabelText("Sitemap URL"), { target: { value: "site.test/sitemap.xml.gz" } });
  fireEvent.click(start());
  await waitFor(() => expect(calls("crawl.start")).toHaveLength(1));
  expect(calls("crawl.start")[0]?.args).toMatchObject({ source: { kind: "list", items: ["https://site.test/sitemap.xml.gz"] } });
  expect(calls("crawl.parseList")).toHaveLength(0);
});

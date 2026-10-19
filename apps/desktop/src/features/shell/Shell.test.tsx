import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createClient, setClient } from "../../ipc/client";
import { MockCore } from "../../ipc/transport/mock";
import { eventBus } from "../../ipc/events";
import { runAction } from "../../ui/actions";
import { allMenuContributions, registerMenuContributions } from "../../ui/menu/registry";
import { setLanguage } from "../../i18n";
import { useLayout, normalizeUiState } from "../../state/layout";
import { QuitDialog } from "./QuitDialog";
import { Shell } from "./Shell";
import { Divider } from "./Divider";
import fixture from "./__fixtures__/ui-state-v1.json";

let core: MockCore;
beforeEach(async () => {
  core = new MockCore();
  core.handlers.set("app.requestQuit", () => ({ kind: "confirm", reasons: ["crawl", "analysis", "export", "mcpSessions"] }));
  core.handlers.set("app.confirmQuit", () => undefined);
  setClient(createClient(core.transport()));
  await setLanguage("en");
  await useLayout.getState().load(1440, 836);
  const state = normalizeUiState(fixture, 1440, 836);
  useLayout.setState({ ...state, loaded: true, error: null });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); setClient(null); });

// spec: UI-021 [P1]
it("cancel preserves all table layouts; confirming updates every open subscriber immediately", async () => {
  const offMenu = registerMenuContributions("features/help", [{ menu: "view", group: "layout", order: 15,
    items: [{ id: "view.resetColumns", labelKey: "shell-reset-columns", action: "view.resetColumns" }] }]);
  const changed = vi.fn();
  const off = useLayout.subscribe(changed);
  render(<div className="os-app"><header /><nav /><main /><aside /><footer /><Shell /></div>);
  expect(allMenuContributions().flatMap((c) => c.items).filter((item) => item.id === "view.resetColumns")).toHaveLength(1);
  act(() => { runAction("view.resetColumns"); });
  expect(screen.getByRole("dialog")).toBeVisible();
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  expect(useLayout.getState().sections.tables).toEqual(fixture.state.tables);
  act(() => { runAction("view.resetColumns"); });
  const before = { ...useLayout.getState().sections };
  fireEvent.click(screen.getByRole("button", { name: /^Reset$/ }));
  const { tables: _tables, ...expected } = before;
  expect(useLayout.getState().sections).toEqual(expected);
  expect(changed).toHaveBeenCalled();
  expect(screen.queryByRole("dialog")).not.toBeInTheDocument();
  off();
  offMenu();
});

// spec: UI-001
it("menu toggles can reopen a collapsed pane and hide an expanded pane without a grip", () => {
  const sidebar = { ...useLayout.getState().layout.sidebar, collapsed: true };
  useLayout.getState().setLayout({ ...useLayout.getState().layout, sidebar });
  render(<div className="os-app"><header /><nav /><main /><aside /><footer /><Shell /></div>);
  act(() => { runAction("view.toggleSidebar"); });
  expect(useLayout.getState().layout.sidebar).toMatchObject({ visible: true, collapsed: false });
  act(() => { runAction("view.toggleSidebar"); });
  expect(useLayout.getState().layout.sidebar.visible).toBe(false);
  expect(screen.queryByRole("separator", { name: /sidebar/ })).not.toBeInTheDocument();
});

// spec: UI-001
it("the default quit action pauses and quits only after confirmation; cancellation is sent explicitly", async () => {
  render(<QuitDialog />);
  act(() => { runAction("app.quit"); });
  const button = await screen.findByRole("button", { name: "Pause and quit" });
  expect(button).toHaveFocus();
  expect(screen.getByText("Crawl analysis in progress")).toBeVisible();
  expect(core.calls.some((c) => c.cmd === "app.confirmQuit")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(core.calls.filter((c) => c.cmd === "app.confirmQuit").map((c) => c.args)).toEqual([{ value: false }]);
  act(() => { runAction("app.quit"); });
  fireEvent.click(await screen.findByRole("button", { name: "Pause and quit" }));
  await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  expect(core.calls.filter((c) => c.cmd === "app.confirmQuit").map((c) => c.args)).toEqual([{ value: false }, { value: true }]);
});

// spec: UI-001
it("native close notices open one confirmation, and a failed command keeps the dialog and saved state", async () => {
  render(<QuitDialog />);
  const notice = { kind: "notice", notice: { level: "info", code: "app-quit-confirmation", args: { reasons: ["export"] } } } as const;
  act(() => { eventBus.emit(notice as never); eventBus.emit(notice as never); });
  expect(screen.getAllByRole("dialog")).toHaveLength(1);
  expect(screen.getByText("Export in progress")).toBeVisible();
  core.handlers.set("app.confirmQuit", () => { throw new Error("checkpoint failed"); });
  fireEvent.click(screen.getByRole("button", { name: "Pause and quit" }));
  await screen.findByRole("alert");
  expect(screen.getByRole("dialog")).toBeVisible();
  expect(useLayout.getState().sections.tables).toEqual(fixture.state.tables);
});

// spec: UI-001
it("keyboard resizing, collapsing, restoring and double-click reset share the separator contract", () => {
  const commit = vi.fn(); const restore = vi.fn(); const reset = vi.fn();
  const { rerender } = render(<Divider pane="detail" label="details" collapsed={false} size={300} viewport={{ width: 1440, height: 836 }}
    onCommit={commit} onRestore={restore} onReset={reset} onPreview={() => undefined} />);
  const separator = screen.getByRole("separator");
  fireEvent.keyDown(separator, { key: "ArrowUp" });
  expect(commit).toHaveBeenLastCalledWith(308);
  fireEvent.keyDown(separator, { key: "Home" });
  expect(commit).toHaveBeenLastCalledWith(0);
  fireEvent.doubleClick(separator);
  expect(reset).toHaveBeenCalledTimes(1);
  rerender(<Divider pane="detail" label="details" collapsed size={0} viewport={{ width: 1440, height: 836 }}
    onCommit={commit} onRestore={restore} onReset={reset} onPreview={() => undefined} />);
  fireEvent.keyDown(separator, { key: "Enter" });
  expect(restore).toHaveBeenCalledTimes(1);
});

// spec: UI-001
it("flushes pending state before an idle quit, and deduplicates simultaneous requests", async () => {
  core.handlers.set("app.requestQuit", () => ({ kind: "quit" }));
  const close = vi.spyOn(window, "close").mockImplementation(() => undefined);
  useLayout.getState().setSection("searchHistory", ["before quit"]);
  render(<QuitDialog />);
  act(() => { runAction("app.quit"); runAction("app.quit"); });
  await waitFor(() => expect(close).toHaveBeenCalledTimes(1));
  expect(core.calls.filter((c) => c.cmd === "app.requestQuit")).toHaveLength(1);
  expect(core.calls.findIndex((c) => c.cmd === "uistate.save")).toBeLessThan(core.calls.findIndex((c) => c.cmd === "app.requestQuit"));
  expect(core.uiState.state).toHaveProperty("searchHistory", ["before quit"]);
});

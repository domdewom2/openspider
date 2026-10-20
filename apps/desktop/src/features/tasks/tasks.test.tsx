import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { createClient, setClient } from "../../ipc/client";
import { setLanguage } from "../../i18n";
import { runAction } from "../../ui/actions";
import { useSession } from "../../state/session";
import { TaskEditor } from "./TaskEditor";
import { TaskManager, TaskHistory, TasksHost } from "./TasksHost";
import { CustomSummary } from "./CustomSummary";
import { useTaskOperations, useTaskWindow } from "./model";
import { taskFixture } from "./fixtures/core";
import fixture from "./fixtures/out-schedule.json";
import type { TaskDef } from "../../ipc/gen/TaskDef";
import type { CustomSummaryArgs } from "../../ipc/gen/CustomSummaryArgs";
import { useState } from "react";

let fx: ReturnType<typeof taskFixture>;
beforeEach(async () => {
  vi.spyOn(Date, "now").mockReturnValue(Date.parse("2026-10-20T12:00:00Z"));
  fx = taskFixture(); setClient(createClient(fx.core.transport("main")));
  useTaskOperations.setState({ handles: {} }); useTaskWindow.setState({ open: false }); useSession.setState({ ops: {} });
  await setLanguage("en");
});
afterEach(() => { cleanup(); setClient(null); vi.restoreAllMocks(); });
const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));
const selectTask = async () => fireEvent.click(await screen.findByRole("checkbox", { name: "Select Nightly" }));
const edit = () => render(<TaskEditor initial={structuredClone(fixture.task) as TaskDef} onClose={vi.fn()} onSaved={vi.fn()} />);

// spec: OUT-160
it("opens from the common menu action and shows all required task columns", async () => {
  render(<TasksHost />);
  act(() => { runAction("tasks.open"); });
  const table = await screen.findByRole("table", { name: "Scheduling" });
  await screen.findByText("https://example.test/");
  expect(within(table).getAllByRole("columnheader").map(h => h.textContent)).toEqual(["Select", "Task name", "Project name (defaults to task name)", "Next run", "Interval", "Mode", "Start URL / source", "Status", "Last run result"]);
  expect(within(table).getByText("Valid")).toBeInTheDocument();
});

// spec: OUT-160
it("has exactly four main tabs and six ordered export subtabs, preserving edits across tab changes", async () => {
  edit(); await screen.findByLabelText("Task name");
  const tabs = screen.getByRole("tablist", { name: "Task settings" });
  expect(within(tabs).getAllByRole("tab").map(x => x.textContent)).toEqual(["General", "Spider", "APIs", "Exports"]);
  fireEvent.change(screen.getByLabelText("Task name"), { target: { value: "Updated" } });
  fireEvent.click(screen.getByRole("tab", { name: "Exports" }));
  expect(within(screen.getByRole("tablist", { name: "Export settings" })).getAllByRole("tab").map(x => x.textContent)).toEqual(["Output settings", "Tables", "Local files", "Looker Studio", "Templates / presets", "Notifications"]);
  fireEvent.click(screen.getByRole("tab", { name: "General" }));
  expect(screen.getByLabelText("Task name")).toHaveValue("Updated");
  expect(screen.queryByLabelText("Headless")).not.toBeInTheDocument();
});

// spec: OUT-161 [P1]
it("sends the monthly 31st and local time to the planner and displays its next run", async () => {
  const saved = vi.fn();
  render(<TaskEditor initial={{ ...structuredClone(fixture.task) as TaskDef, id: null, project: "", schedule: { kind: "monthly", day: 31, time: "02:30" } }} onClose={vi.fn()} onSaved={saved} />);
  click("Save"); await waitFor(() => expect(saved).toHaveBeenCalledOnce());
  const call = fx.core.calls.find(c => c.cmd === "tasks.save")!;
  expect((call.args as { task: TaskDef }).task).toMatchObject({ project: "Nightly", schedule: { kind: "monthly", day: 31, time: "02:30" }, execution: { headless: true } });
  expect(fx.core.calls.find(c => c.cmd === "tasks.register")?.args).toEqual({ id: "task-1", dryRun: false });
});

// spec: OUT-161 [P1]
it("rejects a once-only past time locally and shows backend validation without saving", async () => {
  const view = render(<TaskEditor initial={{ ...fixture.task, schedule: { kind: "once", at: "2020-01-01T03:00:00+01:00" } } as TaskDef} onClose={vi.fn()} onSaved={vi.fn()} />);
  expect(screen.getByRole("button", { name: "Save" })).toBeDisabled(); view.unmount();
  fx.control.issues = [{ path: "name", messageKey: "tasks-invalid", args: {}, warning: false }];
  edit(); click("Save"); expect(await screen.findByRole("alert")).toHaveTextContent("name: Invalid");
  expect(fx.core.calls.some(c => c.cmd === "tasks.save")).toBe(false);
});

// spec: OUT-161 [P1]
it("edits an imported once-only instant in the current zone without shifting its hour", async () => {
  const saved = vi.fn();
  render(<TaskEditor initial={structuredClone(fixture.importedOnce) as TaskDef} onClose={vi.fn()} onSaved={saved} />);
  const input = screen.getByLabelText("Start date and time");
  const date = new Date("2026-11-01T14:00:00Z");
  const pad = (n: number) => String(n).padStart(2, "0");
  const wallTime = `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:00`;
  expect(input).toHaveValue(wallTime);
  click("Save"); await waitFor(() => expect(saved).toHaveBeenCalledOnce());
  const original = (fx.core.calls.find(c => c.cmd === "tasks.save")!.args as { task: TaskDef }).task;
  expect(original.schedule).toEqual(fixture.importedOnce.schedule);
  fireEvent.change(input, { target: { value: wallTime.replace(/:00$/, ":01") } });
  click("Save"); await waitFor(() => expect(saved).toHaveBeenCalledTimes(2));
  const changed = (fx.core.calls.filter(c => c.cmd === "tasks.save")[1].args as { task: TaskDef }).task.schedule;
  expect(changed.kind === "once" && Date.parse(changed.at)).toBe(Date.parse("2026-11-01T14:01:00Z"));
});

// spec: OUT-160
it("offers saved auth profiles separately from configuration and saves edited or cleared references", async () => {
  const saved = vi.fn();
  render(<TaskEditor initial={structuredClone(fixture.task) as TaskDef} onClose={vi.fn()} onSaved={saved} />);
  fireEvent.click(screen.getByRole("tab", { name: "Spider" }));
  const input = screen.getByLabelText(/Auth profile/);
  expect(input).toHaveValue("shop-login");
  await waitFor(() => expect(document.querySelector("datalist option[value='shop-login']")).toBeInTheDocument());
  fireEvent.change(input, { target: { value: "profiles/imported.osauth" } });
  click("Save"); await waitFor(() => expect(saved).toHaveBeenCalledOnce());
  expect(fx.definitions()[0].crawl).toMatchObject({ profile: "shop", authProfile: "profiles/imported.osauth", overrides: { "speed.threads": 10 } });
  fireEvent.change(input, { target: { value: "" } });
  click("Save"); await waitFor(() => expect(saved).toHaveBeenCalledTimes(2));
  expect(fx.definitions()[0].crawl.authProfile).toBeNull();
});

// spec: OUT-160
it("retries a failed OS registration with the saved ID instead of duplicating the task", async () => {
  fx.control.registrationsFail = true;
  render(<TaskEditor initial={{ ...fixture.task, id: null } as TaskDef} onClose={vi.fn()} onSaved={vi.fn()} />);
  click("Save"); await screen.findByRole("alert");
  fx.control.registrationsFail = false; click("Save");
  await waitFor(() => expect(fx.core.calls.filter(c => c.cmd === "tasks.register")).toHaveLength(2));
  expect(fx.definitions().filter(d => d.id !== "nightly")).toHaveLength(1);
});

// spec: OUT-160
// spec: OUT-171 [P1]
it("starts a run and retains the abort handle after closing and reopening", async () => {
  const view = render(<TaskManager onClose={vi.fn()} />); await selectTask(); click("Run now");
  await screen.findByRole("button", { name: "Abort run" }); view.unmount();
  render(<TaskManager onClose={vi.fn()} />); click("Abort run");
  await waitFor(() => expect(fx.core.calls.some(c => c.cmd === "op.cancel")).toBe(true));
  expect(fx.core.calls.find(c => c.cmd === "op.cancel")?.args).toEqual({ opId: "operation-1" });
  expect(fx.core.calls.filter(c => c.cmd === "tasks.run")).toHaveLength(1);
  expect(screen.queryByRole("button", { name: /pause/i })).not.toBeInTheDocument();
});

// spec: OUT-160
it("cancels an already running scheduled crawl from the core mapping after a fresh dialog open", async () => {
  fx.control.lastRun = structuredClone(fixture.runningRun) as import("../../ipc/gen/TaskRunRow").TaskRunRow;
  const view = render(<TaskManager onClose={vi.fn()} />);
  await screen.findByRole("button", { name: "Abort run" });
  expect(useTaskOperations.getState().handles).toEqual({});
  view.unmount();
  render(<TaskManager onClose={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: "Abort run" }));
  await waitFor(() => expect(fx.core.calls.find(c => c.cmd === "op.cancel")?.args).toEqual({ opId: "planned-operation" }));
  expect(fx.core.calls.some(c => c.cmd === "tasks.run")).toBe(false);
  act(() => useSession.setState({ ops: { "planned-operation": { opId: "planned-operation", labelKey: "tasks-op-cancelled", done: 1, state: "cancelled" } } }));
  await waitFor(() => expect(screen.queryByRole("button", { name: "Abort run" })).not.toBeInTheDocument());
});

// spec: OUT-160
// spec: OUT-170 [P1]
it("exports only selected IDs, handles cancelled file dialogs, and refreshes after import completion", async () => {
  const choose = vi.fn().mockResolvedValueOnce(null).mockResolvedValueOnce("tasks.ostask").mockResolvedValueOnce("import.ostask");
  render(<TaskManager onClose={vi.fn()} desktop={{ choose }} />); await selectTask();
  click("Export tasks…"); await waitFor(() => expect(choose).toHaveBeenCalledOnce());
  expect(fx.core.calls.some(c => c.cmd === "tasks.export")).toBe(false);
  click("Export tasks…"); await waitFor(() => expect(fx.core.calls.find(c => c.cmd === "tasks.export")?.args).toEqual({ ids: ["nightly"], path: "tasks.ostask" }));
  act(() => { useSession.setState({ ops: { "operation-1": { opId: "operation-1", labelKey: "tasks-op-done", done: 1, state: "done" } } }); });
  await waitFor(() => expect(screen.getByRole("button", { name: "Import tasks…" })).toBeEnabled()); click("Import tasks…");
  await waitFor(() => expect(fx.core.calls.find(c => c.cmd === "tasks.import")?.args).toEqual({ path: "import.ostask" }));
  const before = fx.core.calls.filter(c => c.cmd === "tasks.list").length;
  act(() => { useSession.setState({ ops: { "operation-2": { opId: "operation-2", labelKey: "tasks-op-done", done: 1, state: "done", result: [] } } }); });
  await waitFor(() => expect(fx.core.calls.filter(c => c.cmd === "tasks.list").length).toBeGreaterThan(before));
});

// spec: OUT-160
it("duplicates with a new ID and only deletes after confirmation", async () => {
  render(<TaskManager onClose={vi.fn()} />); await selectTask(); click("Duplicate");
  expect(screen.getByLabelText("Task name")).toHaveValue("Nightly Copy"); click("Save");
  await waitFor(() => expect(fx.definitions()).toHaveLength(2));
  await waitFor(() => expect(screen.queryByRole("dialog", { name: "Add task" })).not.toBeInTheDocument());
  click("Delete task");
  expect(fx.core.calls.some(c => c.cmd === "tasks.delete")).toBe(false);
  const confirm = screen.getByRole("dialog", { name: "Delete task" });
  fireEvent.click(within(confirm).getByRole("button", { name: "Delete task" }));
  await waitFor(() => expect(fx.definitions()).toHaveLength(1));
});

// spec: OUT-169 [P1]
it("shows every history column and opens the selected run's log", async () => {
  render(<TaskHistory id="nightly" onClose={vi.fn()} />);
  expect(await screen.findByText("Completed with warnings")).toBeInTheDocument();
  expect(screen.getByText("Aborted")).toBeInTheDocument();
  expect(screen.getByText("60 s")).toBeInTheDocument(); expect(screen.getByText("crawl-list")).toBeInTheDocument();
  expect(screen.getByText("output/internal.csv")).toBeInTheDocument(); expect(screen.getByText("PSI unavailable")).toBeInTheDocument();
  expect(fx.core.calls.find(c => c.cmd === "tasks.history")?.args).toEqual({ id: "nightly", offset: 0, limit: 100 });
  expect(screen.getAllByRole("columnheader").map(h => h.textContent)).toEqual(["Started", "Finished", "Duration", "Result", "URLs crawled", "Crawl ID", "Output folder", "Output files", "Message", "Open run log"]);
  expect(screen.getByText("1,234")).toBeInTheDocument();
  expect(screen.getByText("0")).toBeInTheDocument();
  expect(screen.getByText("output/list-run")).toBeInTheDocument();
  expect(screen.getByText("output/aborted-run")).toBeInTheDocument();
  const warningRow = screen.getByText("crawl-list").closest("tr")!;
  fireEvent.click(within(warningRow).getByRole("button", { name: "Open run log" }));
  const abortedRow = screen.getByText("saved-partial").closest("tr")!;
  fireEvent.click(within(abortedRow).getByRole("button", { name: "Open run log" }));
  await waitFor(() => expect(fx.core.calls.filter(c => c.cmd === "tasks.openLog").map(c => c.args)).toEqual([{ id: "run-list" }, { id: "run-aborted" }]));
  expect(fx.core.calls.some(c => c.cmd === "diag.openLogs")).toBe(false);
});

function SummaryHarness({ initial = { account: "google", target: "report", metrics: [] } }: { initial?: CustomSummaryArgs }) {
  const [value, setValue] = useState<CustomSummaryArgs | null>(initial);
  return <><CustomSummary value={value} onChange={setValue} /><output data-testid="summary-value">{JSON.stringify(value)}</output></>;
}

// spec: OUT-210 [M2]
// spec: OUT-211 [M2]
it("loads registry metrics, preserves chosen column order, and imports/exports canonical text lists", async () => {
  render(<SummaryHarness />);
  click(await screen.findByRole("button", { name: "Internal:HTML" }).then(b => b.textContent!));
  click("Response Codes:Client Error (4xx)");
  click("Move Response Codes:Client Error (4xx) up");
  click("Show selection as text");
  expect(screen.getByLabelText(/Metric names/)).toHaveValue("Response Codes:Client Error (4xx)\nInternal:HTML");
  fireEvent.change(screen.getByLabelText(/Metric names/), { target: { value: "Internal:HTML\nInternal:HTML\nAverage:Response Time" } }); click("Use text list");
  expect(JSON.parse(screen.getByTestId("summary-value").textContent!).metrics).toEqual(["Internal:HTML", "Average:Response Time"]);
  expect(screen.getByText(/Fixed columns: Date, Start URL/)).toBeInTheDocument();
});

// spec: OUT-211 [M2]
it("keeps unknown saved metrics visible with a named warning and an explicit remove action", async () => {
  render(<SummaryHarness initial={{ account: "google", target: "report", metrics: ["Old:Metric"] }} />);
  expect(await screen.findByRole("status")).toHaveTextContent("Old:Metric");
  expect(screen.getByTestId("summary-value")).toHaveTextContent("Old:Metric");
  click("Remove Old:Metric"); expect(JSON.parse(screen.getByTestId("summary-value").textContent!).metrics).toEqual([]);
});

// spec: OUT-160
// spec: OUT-210 [M2]
it("saves ordered summary metrics and output changes while preserving crawl overrides", async () => {
  const saved = vi.fn();
  render(<TaskEditor initial={structuredClone(fixture.task) as TaskDef} onClose={vi.fn()} onSaved={saved} />);
  fireEvent.click(screen.getByRole("tab", { name: "Exports" }));
  fireEvent.change(screen.getByLabelText("Output folder"), { target: { value: "exports/nightly" } });
  fireEvent.click(screen.getByRole("tab", { name: "Looker Studio" }));
  fireEvent.click(screen.getByRole("checkbox", { name: "Custom Crawl Overview" }));
  fireEvent.change(screen.getByLabelText("Google account"), { target: { value: "google-work" } });
  fireEvent.change(screen.getByLabelText("Summary target"), { target: { value: "shop-summary" } });
  fireEvent.click(await screen.findByRole("button", { name: "Internal:HTML" }));
  click("Save"); await waitFor(() => expect(saved).toHaveBeenCalledOnce());
  const request = (fx.core.calls.find(c => c.cmd === "tasks.save")!.args as { task: TaskDef }).task;
  expect(request.customSummary).toEqual({ account: "google-work", target: "shop-summary", metrics: ["Internal:HTML"] });
  expect(request.exportOptions?.dir).toBe("exports/nightly");
  expect(request.crawl.overrides).toEqual({ "speed.threads": 10 });
});

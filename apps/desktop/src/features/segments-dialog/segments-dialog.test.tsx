import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { createQueryClient, Providers } from "../../app/Providers";
import { setLanguage } from "../../i18n";
import { createClient, setClient } from "../../ipc/client";
import { MockCore } from "../../ipc/transport/mock";
import { useSession } from "../../state/session";
import type { SegmentDef } from "../../ipc/gen/SegmentDef";
import type { SegmentPreviewArgs } from "../../ipc/gen/SegmentPreviewArgs";
import type { SegmentSaveArgs } from "../../ipc/gen/SegmentSaveArgs";
import type { ValidationIssue } from "../../ipc/gen/ValidationIssue";
import { runAction } from "../../ui/actions";
import fixture from "../../../e2e/sidebar-fach/fach-dto.json";
import { SegmentsDialogHost } from "./SegmentsDialog";
import { closeSegmentsDialog, openSegmentsDialog, useSegmentsDialog } from "./store";
import { conditionError, emptyCondition, exportJson, fromGroups, hasBlockingError, importJson, issueTarget, move, nameError, newSegment, toGroups } from "./model";

// fx-ui-dirs: six URLs; the mock core evaluates the rules in list order, first match wins (CRAWL-134).
const URLS = [
  { address: "https://site.test/blog/a", words: 500 }, { address: "https://site.test/blog/b", words: 400 }, { address: "https://site.test/blog/c", words: 100 },
  { address: "https://site.test/shop/x", words: 100 }, { address: "https://site.test/shop/y", words: 900 }, { address: "https://site.test/about", words: 50 },
];
const matches = (s: SegmentDef, u: (typeof URLS)[number]) => toGroups(s.rules).some(g => g.every(c => {
  const v = c.column === "Address" ? u.address : String(u.words);
  return c.op === "contains" ? v.includes(c.value ?? "") : c.op === "lessThan" ? Number(v) < Number(c.value) : false;
}));
function assign(list: SegmentDef[]) {
  const counts = new Map(list.map(s => [s.id, 0]));
  let unassigned = 0;
  for (const u of URLS) { const hit = list.find(s => matches(s, u)); if (hit) counts.set(hit.id, counts.get(hit.id)! + 1); else unassigned++; }
  return { counts: [...counts].map(([id, urls]) => ({ id, urls })), unassigned };
}

let queryClient: ReturnType<typeof createQueryClient>;
let core: MockCore;
let saved: SegmentSaveArgs | null;
let issues: (segment: SegmentDef) => ValidationIssue[];
let opSeq = 0;
const original = () => structuredClone(fixture.segmentList) as SegmentDef[];

beforeEach(async () => {
  queryClient = createQueryClient();
  saved = null;
  issues = () => [];
  core = new MockCore();
  core.handlers.set("segment.list", () => original());
  core.handlers.set("catalog.columns", () => fixture.columns);
  core.handlers.set("segment.validate", args => issues((args as { segment: SegmentDef }).segment));
  core.handlers.set("segment.preview", args => {
    const opId = `preview-${++opSeq}`;
    const result = assign((args as SegmentPreviewArgs).segments);
    queueMicrotask(() => useSession.setState(s => ({ ops: { ...s.ops, [opId]: { opId, labelKey: "op-segment-preview", done: 1, total: 1, state: "done", result } } })));
    return { opId, labelKey: "op-segment-preview", cancellable: true };
  });
  core.handlers.set("segment.save", args => { saved = args as SegmentSaveArgs; return { invalidatedSteps: [] }; });
  core.handlers.set("segment.reassign", () => {
    queueMicrotask(() => useSession.setState(s => ({ ops: { ...s.ops, "reassign-1": { opId: "reassign-1", labelKey: "op-segment-reassign", done: 1, total: 1, state: "done" } } })));
    return { opId: "reassign-1", labelKey: "op-segment-reassign", cancellable: true };
  });
  core.handlers.set("op.cancel", () => undefined);
  setClient(createClient(core.transport()));
  useSession.setState({ ops: {} });
  closeSegmentsDialog();
  await setLanguage("en");
});
afterEach(() => { cleanup(); closeSegmentsDialog(); setClient(null); queryClient.clear(); vi.restoreAllMocks(); });

async function open() {
  render(<Providers queryClient={queryClient}><SegmentsDialogHost /></Providers>);
  act(() => openSegmentsDialog());
  const list = await screen.findByRole("list", { name: "Segments in priority order" });
  await within(list).findByRole("button", { name: /^Blog/ });
  return list;
}
const ok = () => screen.getByRole("button", { name: "OK" });
const names = (list: HTMLElement) => within(list).getAllByRole("listitem").map(li => li.querySelector(".os-segdlg-name")?.textContent);
const counts = (list: HTMLElement) => within(list).getAllByRole("listitem").map(li => li.querySelector("[data-preview-count]")?.textContent);
const selectSegment = (name: string) => fireEvent.click(screen.getByRole("button", { name: new RegExp(`^${name}`) }));
const errors = () => screen.queryAllByRole("alert").map(a => a.textContent);

// spec: UI-068
it("lists the segments in priority order with the preview counters of the core and the first-match rule", async () => {
  const list = await open();
  expect(names(list)).toEqual(["Blog", "Thin pages", "Rest"]);
  await waitFor(() => expect(counts(list)).toEqual(["3 URLs", "2 URLs", "1 URLs"]));
  expect(screen.getByText("Without segment: 0 URLs")).toBeVisible();
  await waitFor(() => expect(ok()).toBeEnabled());
});

// spec: UI-068
it("changes the preview immediately when a segment is moved and renumbers the priority", async () => {
  const list = await open();
  await waitFor(() => expect(counts(list)[0]).toBe("3 URLs"));
  fireEvent.click(within(list).getByRole("button", { name: "Move Rest up" }));
  fireEvent.click(within(list).getByRole("button", { name: "Move Rest up" }));
  expect(names(list)).toEqual(["Rest", "Blog", "Thin pages"]);
  await waitFor(() => expect(counts(list)).toEqual(["6 URLs", "0 URLs", "0 URLs"]), { timeout: 3000 });
  const last = core.calls.filter(c => c.cmd === "segment.preview").at(-1)?.args as SegmentPreviewArgs;
  expect(last.segments.map(s => [s.name, s.order])).toEqual([["Rest", 0], ["Blog", 1], ["Thin pages", 2]]);
  expect(move(original(), 2, 0).map(s => s.order)).toEqual([0, 1, 2]);
});

// spec: UI-068
it("marks names while typing: empty, longer than 64 characters and duplicates without regard to case", async () => {
  const list = await open();
  const name = () => screen.getByRole("textbox", { name: "Name" });
  fireEvent.change(name(), { target: { value: "" } });
  expect(errors()).toContain("The name must not be empty.");
  expect(ok()).toBeDisabled();
  fireEvent.change(name(), { target: { value: "x".repeat(65) } });
  expect(errors()).toContain("The name may have at most 64 characters.");
  fireEvent.change(name(), { target: { value: "REST" } });
  expect(errors()).toContain("Another segment already has this name (case is ignored).");
  expect(ok()).toBeDisabled();
  fireEvent.change(name(), { target: { value: "Blog 2" } });
  expect(errors().filter(Boolean)).toEqual([]);
  await waitFor(() => expect(ok()).toBeEnabled(), { timeout: 3000 });
  expect(names(list)[0]).toBe("Blog 2");
  const dup = [{ ...original()[0]!, name: "Blog" }, { ...original()[1]!, name: "blog" }];
  expect(nameError(dup, 1)).toEqual({ key: "sgd-err-name-duplicate" });
  expect(nameError([{ ...dup[0]!, name: "x".repeat(64) }], 0)).toBeNull();
});

// spec: UI-068
it("rejects “is in segment” on a segment further down at the condition and releases OK after moving it up", async () => {
  const list = await open();
  // Blog (top) refers to Rest (below): invalid.
  fireEvent.click(screen.getByRole("button", { name: "Add condition (AND)" }));
  const cond2 = screen.getByRole("group", { name: "Condition 2 of group 1" });
  fireEvent.change(within(cond2).getByRole("combobox", { name: "Column" }), { target: { value: "Segment" } });
  fireEvent.change(within(cond2).getByRole("combobox", { name: "Operator" }), { target: { value: "equals" } });
  fireEvent.change(within(cond2).getByRole("textbox", { name: "Value" }), { target: { value: "Rest" } });
  expect(within(cond2).getByRole("alert")).toHaveTextContent("Rest must be listed above this segment.");
  expect(within(cond2).getByRole("textbox", { name: "Value" })).toHaveAttribute("aria-invalid", "true");
  expect(ok()).toBeDisabled();
  // Moving Rest above Blog resolves it.
  fireEvent.click(within(list).getByRole("button", { name: "Move Rest up" }));
  fireEvent.click(within(list).getByRole("button", { name: "Move Rest up" }));
  expect(within(screen.getByRole("group", { name: "Condition 2 of group 1" })).queryByRole("alert")).toBeNull();
  await waitFor(() => expect(ok()).toBeEnabled(), { timeout: 3000 });
  const segs = original();
  const blog = { ...segs[0]!, rules: fromGroups([[emptyCondition(), { column: "Segment", op: "equals" as const, value: "Rest" }]]) };
  expect(conditionError([blog, segs[1]!, segs[2]!], 0, { column: "Segment", op: "equals", value: "Rest" })).toEqual({ key: "sgd-err-segment-below", args: { name: "Rest" } });
  expect(conditionError([segs[2]!, blog], 1, { column: "Segment", op: "equals", value: "Rest" })).toBeNull();
  expect(conditionError(segs, 0, { column: "Segment", op: "equals", value: "Nope" })).toEqual({ key: "sgd-err-segment-unknown", args: { name: "Nope" } });
});

// spec: UI-068
it("shows a regex error of the core with its position at the condition and blocks OK until it is fixed", async () => {
  issues = s => s.id === "seg-blog" && toGroups(s.rules)[0]![0]!.value === "(["
    ? [{ path: "segments.list[0].groups[0].conditions[0].value", messageKey: "err-regex", args: {}, line: 1, column: 2, warning: false }]
    : [];
  await open();
  const cond = screen.getByRole("group", { name: "Condition 1 of group 1" });
  fireEvent.change(within(cond).getByRole("combobox", { name: "Operator" }), { target: { value: "regex" } });
  fireEvent.change(within(cond).getByRole("textbox", { name: "Value" }), { target: { value: "([" } });
  expect(await within(cond).findByRole("alert", {}, { timeout: 3000 })).toHaveTextContent("(line 1, column 2)");
  expect(ok()).toBeDisabled();
  fireEvent.change(within(cond).getByRole("textbox", { name: "Value" }), { target: { value: "blog" } });
  await waitFor(() => expect(within(cond).queryByRole("alert")).toBeNull(), { timeout: 3000 });
  await waitFor(() => expect(ok()).toBeEnabled(), { timeout: 3000 });
  expect(issueTarget("segments.list[3].groups[1].conditions[4].value")).toEqual({ kind: "condition", group: 1, condition: 4 });
  expect(issueTarget("rules.children[0].children[2]")).toEqual({ kind: "condition", group: 0, condition: 2 });
  expect(issueTarget("segments.list[0].name")).toEqual({ kind: "name" });
  const warning: ValidationIssue = { path: "x", messageKey: "w", args: {}, warning: true };
  expect(hasBlockingError(original(), 0, [warning])).toBe(false);
  expect(hasBlockingError(original(), 0, [{ ...warning, warning: false }])).toBe(true);
});

// spec: UI-068
it("flags conditions on analysis or API columns as pending", async () => {
  await open();
  selectSegment("Thin pages");
  const cond = screen.getByRole("group", { name: "Condition 1 of group 1" });
  expect(within(cond).queryByText("pending")).toBeNull();
  fireEvent.change(within(cond).getByRole("combobox", { name: "Column" }), { target: { value: "Link Score" } });
  expect(within(cond).getByText("pending")).toBeVisible();
});

// spec: UI-068
it("round-trips the segment list through JSON export and import (order, colours, rules)", async () => {
  const list = await open();
  fireEvent.click(screen.getByText("Import and export (JSON)"));
  fireEvent.click(screen.getByRole("button", { name: "Export" }));
  const box = screen.getByRole("textbox", { name: "Segment list as JSON" }) as HTMLTextAreaElement;
  expect(JSON.parse(box.value)).toEqual(original());
  const exported = box.value;
  fireEvent.click(within(list).getByRole("button", { name: "Delete Blog" }));
  fireEvent.click(within(list).getByRole("button", { name: "Move Rest up" }));
  expect(names(list)).toEqual(["Rest", "Thin pages"]);
  fireEvent.change(box, { target: { value: exported } });
  fireEvent.click(screen.getByRole("button", { name: "Import" }));
  expect(names(list)).toEqual(["Blog", "Thin pages", "Rest"]);
  await waitFor(() => expect(ok()).toBeEnabled(), { timeout: 3000 });
  fireEvent.click(ok());
  await waitFor(() => expect(saved).not.toBeNull());
  expect(saved!.segments).toEqual(original());
  expect(importJson(exportJson(original()))).toEqual({ ok: true, segments: original() });
  expect(importJson("{").ok).toBe(false);
  expect(importJson('[{"id":"a"}]').ok).toBe(false);
});

// spec: UI-068
it("shows an error for unusable JSON and keeps the list", async () => {
  const list = await open();
  fireEvent.click(screen.getByText("Import and export (JSON)"));
  fireEvent.change(screen.getByRole("textbox", { name: "Segment list as JSON" }), { target: { value: "[1]" } });
  fireEvent.click(screen.getByRole("button", { name: "Import" }));
  expect(screen.getByText(/The JSON cannot be imported/)).toBeVisible();
  expect(names(list)).toEqual(["Blog", "Thin pages", "Rest"]);
});

// spec: UI-068
it("saves the full list in priority order with chosen colours, then reassigns without a recrawl and closes", async () => {
  const list = await open();
  fireEvent.click(screen.getByRole("button", { name: "#AA54DB" }));
  fireEvent.click(screen.getByRole("button", { name: "Add segment" }));
  fireEvent.change(screen.getByRole("textbox", { name: "Name" }), { target: { value: "Shop" } });
  fireEvent.change(screen.getByRole("textbox", { name: "Value" }), { target: { value: "/shop/" } });
  expect(names(list)).toEqual(["Blog", "Thin pages", "Rest", "Shop"]);
  await waitFor(() => expect(ok()).toBeEnabled(), { timeout: 3000 });
  fireEvent.click(ok());
  await waitFor(() => expect(core.calls.some(c => c.cmd === "segment.reassign")).toBe(true));
  expect(saved!.segments.map(s => [s.name, s.order])).toEqual([["Blog", 0], ["Thin pages", 1], ["Rest", 2], ["Shop", 3]]);
  expect(saved!.segments[0]!.color).toBe("#AA54DB");
  expect(saved!.segments[3]!.rules).toEqual({ kind: "group", op: "or", children: [{ kind: "group", op: "and", children: [{ kind: "cond", column: "Address", op: "contains", value: "/shop/" }] }] });
  await waitFor(() => expect(useSegmentsDialog.getState().open).toBe(false));
  const sequence = core.calls.map(c => c.cmd).filter(c => c === "segment.save" || c === "segment.reassign");
  expect(sequence).toEqual(["segment.save", "segment.reassign"]);
});

// spec: UI-068
it("waits for reassignment to finish before invalidating sidebar counters and closing", async () => {
  core.handlers.set("segment.reassign", () => ({ opId: "reassign-slow", labelKey: "op-segment-reassign", cancellable: false }));
  queryClient.setQueryData(["sidebar", "segments"], fixture.segmentRows.indexability);
  const invalidate = vi.spyOn(queryClient, "invalidateQueries");
  await open();
  await waitFor(() => expect(ok()).toBeEnabled());
  fireEvent.click(ok());
  await waitFor(() => expect(core.calls.some(c => c.cmd === "segment.reassign")).toBe(true));
  const status = { opId: "reassign-slow", labelKey: "op-segment-reassign", done: 0, total: 1, state: "running" as const };
  act(() => useSession.setState(s => ({ ops: { ...s.ops, [status.opId]: status } })));
  expect(useSegmentsDialog.getState().open).toBe(true);
  expect(ok()).toBeDisabled();
  expect(invalidate).not.toHaveBeenCalled();
  expect(queryClient.getQueryState(["sidebar", "segments"])?.isInvalidated).toBe(false);
  act(() => useSession.setState(s => ({ ops: { ...s.ops, [status.opId]: { ...status, state: "done", done: 1 } } })));
  await waitFor(() => expect(useSegmentsDialog.getState().open).toBe(false));
  expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: ["sidebar"] });
  expect(queryClient.getQueryState(["sidebar", "segments"])?.isInvalidated).toBe(true);
});

// spec: UI-068
it.each(["failed", "cancelled"] as const)("keeps the dialog open after reassignment is %s and permits retry", async state => {
  core.handlers.set("segment.reassign", () => ({ opId: "reassign-error", labelKey: "op-segment-reassign", cancellable: true }));
  const invalidate = vi.spyOn(queryClient, "invalidateQueries");
  await open();
  await waitFor(() => expect(ok()).toBeEnabled());
  fireEvent.click(ok());
  await waitFor(() => expect(core.calls.some(c => c.cmd === "segment.reassign")).toBe(true));
  act(() => useSession.setState(s => ({ ops: { ...s.ops, "reassign-error": {
    opId: "reassign-error", labelKey: "op-segment-reassign", done: 0, total: 1, state,
    error: state === "failed" ? { code: "Internal", messageKey: "err-internal", args: {}, retryable: true } : null,
  } } })));
  await waitFor(() => expect(screen.getAllByRole("alert").length).toBeGreaterThan(0));
  expect(useSegmentsDialog.getState().open).toBe(true);
  expect(invalidate).not.toHaveBeenCalled();
  await waitFor(() => expect(ok()).toBeEnabled());
  core.handlers.set("segment.reassign", () => {
    const opId = "reassign-retry";
    useSession.setState(s => ({ ops: { ...s.ops, [opId]: { opId, labelKey: "op-segment-reassign", done: 1, total: 1, state: "done" } } }));
    return { opId, labelKey: "op-segment-reassign", cancellable: true };
  });
  fireEvent.click(ok());
  await waitFor(() => expect(useSegmentsDialog.getState().open).toBe(false));
  expect(invalidate).toHaveBeenCalledExactlyOnceWith({ queryKey: ["sidebar"] });
});

// spec: UI-068
it("keeps the dialog open and shows the error when saving fails; Cancel discards the draft", async () => {
  core.handlers.set("segment.save", () => { throw Object.assign(new Error("x"), { cmdError: true }); });
  await open();
  await waitFor(() => expect(ok()).toBeEnabled(), { timeout: 3000 });
  fireEvent.click(ok());
  await waitFor(() => expect(core.calls.some(c => c.cmd === "segment.save")).toBe(true));
  await waitFor(() => expect(screen.getAllByRole("alert").length).toBeGreaterThan(0));
  expect(useSegmentsDialog.getState().open).toBe(true);
  fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(useSegmentsDialog.getState().open).toBe(false));
  expect(core.calls.some(c => c.cmd === "segment.reassign")).toBe(false);
});

// spec: UI-068
it("opens through the action segments.configure and builds new segments with a free palette colour", async () => {
  render(<Providers queryClient={queryClient}><SegmentsDialogHost /></Providers>);
  expect(screen.queryByRole("dialog")).toBeNull();
  act(() => { runAction("segments.configure"); });
  expect(await screen.findByRole("dialog", { name: "Configure segments" })).toBeVisible();
  const first = newSegment([]);
  const second = newSegment([first]);
  expect(second.color).not.toBe(first.color);
  expect(second.id).not.toBe(first.id);
  expect(toGroups(first.rules)).toEqual([[emptyCondition()]]);
});

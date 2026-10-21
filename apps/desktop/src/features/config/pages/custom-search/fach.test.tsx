import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { setLanguage } from "../../../../i18n";
import { createClient, setClient } from "../../../../ipc/client";
import { MockCore } from "../../../../ipc/transport/mock";
import type { CrawlConfigDto } from "../../../../ipc/gen/CrawlConfigDto";
import type { ValidationIssue } from "../../../../ipc/gen/ValidationIssue";
import { useSession } from "../../../../state/session";
import { ConfigHost } from "../../frame/ConfigHost";
import { closeConfig, openConfig } from "../../frame/store";
import { installConfigCore, type FakeConfigCore } from "../../frame/testing";
import { getPath } from "../../frame/model";
import fixture from "./fixtures/fach.json";
import { bulkAdd, importExpressions, newFilter } from "./model";
import { DEFAULT_RULES } from "../link-positions";
import { ELEMENTS } from "../change-detection";
import changeElements from "../change-detection/fixtures/elements.json";
import { importSegments, previewDefs } from "../segments/model";
import { changeMode } from "../custom-extraction";

let core: MockCore; let fake: FakeConfigCore;
const change = (el: HTMLElement, value: string) => fireEvent.change(el, { target: { value } });
async function open(anchor: string) { act(() => openConfig({ anchor })); return screen.findByRole("dialog", { name: "Crawl configuration" }); }
async function checked() { await waitFor(() => expect(screen.getByRole("button", { name: "OK" })).toBeEnabled()); }
beforeEach(async () => {
  core = new MockCore(); fake = installConfigCore(core); fake.config = { ...fake.config, ...structuredClone(fixture) };
  core.handlers.set("compare.contentAvailable", () => false);
  core.handlers.set("api.list", () => []);
  core.handlers.set("catalog.columns", () => []);
  setClient(createClient(core.transport("main"))); await setLanguage("en");
  useSession.setState({ crawl: null, ops: {}, analysis: null }); render(<ConfigHost />);
});
afterEach(() => { act(() => closeConfig()); cleanup(); });

describe("Fachseiten against the configuration fixture", () => {
  // spec: INTEG-020
  it("edits multiline filters in a draft, saves their order and discards on cancel", async () => {
    const d = await open("custom_search");
    change(within(d).getByRole("textbox", { name: "Expression" }), "line one\nline two");
    expect(fake.setCalls).toHaveLength(0); await checked();
    fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    expect((fake.config.custom_search as typeof fixture.custom_search).filters[0]?.expression).toBe("line one\nline two");
    const again = await open("custom_search"); change(within(again).getByRole("textbox", { name: "Expression" }), "discard");
    fireEvent.click(within(again).getByRole("button", { name: "Cancel" })); expect(fake.setCalls).toHaveLength(1);
  });
  // spec: INTEG-026
  it("exposes XPath only for the XPath search scope and persists it", async () => {
    const d = await open("custom_search");
    change(within(d).getByLabelText("Search scope"), "xpath");
    change(within(d).getByRole("textbox", { name: "XPath" }), "//main"); await checked();
    fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    expect((fake.config.custom_search as typeof fixture.custom_search).filters[0]).toMatchObject({ scope: "xpath", xpath: "//main" });
  });
  // spec: INTEG-028
  it("passes PCRE flags and lookaround unchanged to config.validate without JS regex evaluation", async () => {
    const d = await open("custom_search"); change(within(d).getByLabelText("Type"), "regex");
    change(within(d).getByRole("textbox", { name: "Expression" }), "(?is)(?=.*rot)(?=.*blau)"); await checked();
    const calls = core.calls.filter(c => c.cmd === "config.validate");
    expect(JSON.stringify(calls.at(-1)?.args)).toContain("(?is)(?=.*rot)(?=.*blau)");
    expect(within(d).queryByRole("checkbox", { name: "Case sensitive" })).toBeNull();
  });
  // spec: INTEG-032
  it("bulk adds with shared options and reports blank, duplicate and overflow rows", () => {
    const rows = Array.from({ length: 98 }, (_, i) => ({ ...newFilter(i + 1), expression: `existing-${i}` }));
    const r = bulkAdd(rows, importExpressions('alpha\n\nalpha\n"beta"\ngamma'), { ...newFilter(1), contains: false, kind: "regex", scope: "xpath", xpath: "//main" });
    expect(r).toMatchObject({ skipped: 2, overflow: 1 });
    expect(r.rows.slice(98)).toEqual(["alpha", "beta"].map(expression => ({ ...newFilter(1), name: expression, expression, contains: false, kind: "regex", scope: "xpath", xpath: "//main" })));
    expect(importExpressions('"one\ntwo"\r\n"a""b"')).toEqual(["one\ntwo", 'a"b']);
  });
  // spec: INTEG-032
  it("applies the bulk draft and its case option from the separate bulk editor", async () => {
    const d = await open("custom_search"); fireEvent.click(within(d).getByRole("button", { name: "Bulk add search filters" }));
    const bulk = within(screen.getByRole("dialog", { name: "Bulk add search filters" })).getByRole("group", { name: "Bulk add search filters" });
    change(within(bulk).getByRole("textbox", { name: "One expression per line" }), "one\none\ntwo");
    fireEvent.click(within(bulk).getByRole("checkbox", { name: "Case sensitive" }));
    fireEvent.click(within(bulk).getByRole("button", { name: "Import" }));
    expect(within(d).getByText(/Skipped 1 empty or duplicate/)).toBeInTheDocument(); await checked();
    fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    expect((fake.config.custom_search as typeof fixture.custom_search).filters.slice(1).map(r => r.case_sensitive)).toEqual([true, true]);
  });
  // spec: INTEG-040
  it("adds extractors with defaults and restores them when reopening", async () => {
    const d = await open("custom_extraction"); fireEvent.click(within(d).getByRole("button", { name: "Add extractor" }));
    expect(within(d).getByRole("group", { name: "Extractor 2" })).toBeInTheDocument();
    change(within(within(d).getByRole("group", { name: "Extractor 2" })).getByRole("textbox", { name: "Expression" }), "//h2");
    await checked(); fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    const again = await open("custom_extraction"); expect(within(again).getByRole("group", { name: "Extractor 2" })).toBeInTheDocument();
  });
  // spec: INTEG-043
  it("supports CSS attributes and disables ignored output controls", async () => {
    const d = await open("custom_extraction"); change(within(d).getByLabelText("Mode"), "css");
    change(within(d).getByRole("textbox", { name: "Attribute (optional)" }), "data-price");
    expect(within(d).getByLabelText("Output")).toBeDisabled(); await checked();
    fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    expect((fake.config.custom_extraction as typeof fixture.custom_extraction).extractors[0]).toMatchObject({ mode: "css", attribute: "data-price" });
  });
  // spec: INTEG-048
  it("offers function values only for XPath and explains the first-node fallback", async () => {
    const d = await open("custom_extraction"); change(within(d).getByLabelText("Output"), "function_value");
    expect(within(d).getByText(/node set returns/)).toBeInTheDocument();
    change(within(d).getByLabelText("Mode"), "css"); expect(within(d).queryByRole("option", { name: "Function value" })).toBeNull();
    expect(changeMode({ name: "x", mode: "xpath", expression: "count(//h1)", output: "function_value", attribute: null }, "regex").output).toBe("text");
  });
  // spec: INTEG-051
  it("shows core syntax errors with position and disables OK even after navigating away", async () => {
    const validate = core.handlers.get("config.validate")!;
    core.handlers.set("config.validate", async (args, ctx) => {
      const cfg = (args as CrawlConfigDto).config as Record<string, unknown>;
      const bad = JSON.stringify(cfg.custom_extraction).includes("//div[");
      return bad ? [{ path: "custom_extraction.extractors[0].expression", messageKey: "err-config-validation", args: { reason: "XPath syntax at position 6" }, line: 1, column: 6, warning: false } satisfies ValidationIssue] : validate(args, ctx);
    });
    const d = await open("custom_extraction"); change(within(d).getByRole("textbox", { name: "Expression" }), "//div[");
    await within(d).findAllByText(/XPath syntax at position 6/); expect(within(d).getByRole("button", { name: "OK" })).toBeDisabled();
    expect(within(d).getByRole("textbox", { name: "Expression" })).toHaveAttribute("aria-invalid", "true");
    expect(within(d).getByText("✗ Invalid")).toHaveAttribute("title", expect.stringContaining("1:6"));
    fireEvent.click(within(d).getByRole("button", { name: "Crawl analysis" }));
    expect(within(d).getByRole("button", { name: "OK" })).toBeDisabled();
  });
  // spec: INTEG-051
  it("does not display a stale successful check as valid while a new expression is pending", async () => {
    const d = await open("custom_extraction"); await checked();
    core.handlers.set("config.validate", () => new Promise(() => {}));
    change(within(d).getByRole("textbox", { name: "Expression" }), "for $x in //a return $x/@href");
    expect(within(d).getByText("Checking …")).toBeInTheDocument();
    expect(within(d).getByRole("button", { name: "OK" })).toBeDisabled();
  });
  // spec: INTEG-060
  it("warns in text mode and edits snippets, MIME filters and action timeouts", async () => {
    const d = await open("custom_js"); expect(within(d).getByText(/require JavaScript rendering/)).toBeInTheDocument();
    const mime = within(d).getByRole("textbox", { name: "Content types (comma separated)" });
    change(mime, "text/html,"); expect(mime).toHaveValue("text/html,");
    change(mime, "text/html, application/json");
    change(within(d).getByLabelText("Type"), "action");
    change(within(d).getByLabelText("Timeout (seconds)"), "8");
    change(within(d).getByRole("textbox", { name: "JavaScript code" }), "window.scrollTo(0, 1000);");
    await checked(); fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    expect((fake.config.custom_js as typeof fixture.custom_js).snippets[0]).toMatchObject({ kind: "action", timeout_s: 8, code: "window.scrollTo(0, 1000);", content_types: ["text/html", "application/json"] });
  });
  // spec: INTEG-081
  it("keeps Content last and restores the ordered standard rules", async () => {
    const d = await open("link_positions"); const groups = within(d).getAllByRole("group");
    expect(groups.map(g => g.querySelector("legend")?.textContent)).toEqual(DEFAULT_RULES.map(r => r.position));
    const content = within(d).getByRole("group", { name: "Content" });
    expect(within(content).getByRole("button", { name: "Move up" })).toBeDisabled();
    fireEvent.click(within(d).getByRole("button", { name: "Add position rule" }));
    fireEvent.click(within(d).getByRole("button", { name: "Reset to defaults" }));
    expect(within(d).getAllByRole("group")).toHaveLength(5);
  });
  // spec: INTEG-080
  // spec: INTEG-082
  it("inserts a custom link position before Content and saves rule priority", async () => {
    const d = await open("link_positions"); fireEvent.click(within(d).getByRole("button", { name: "Add position rule" }));
    const group = within(d).getAllByRole("group")[4]!;
    change(within(group).getByRole("textbox", { name: "Position" }), "Breadcrumb");
    change(within(group).getByRole("textbox", { name: "Substrings (one per line)" }), "breadcrumb");
    fireEvent.click(within(group).getByRole("button", { name: "Move up" })); await checked();
    fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    const rules = getPath(fake.config, "link_positions.rules") as typeof DEFAULT_RULES;
    expect(rules[3]).toEqual({ position: "Breadcrumb", contains: ["breadcrumb"] }); expect(rules.at(-1)?.position).toBe("Content");
  });
  // spec: CRAWL-134
  it("round trips config segments with all operators and converts inclusive numeric rules for previews", () => {
    const segments = importSegments(JSON.stringify(fixture.segments.list)); expect(segments).toEqual(fixture.segments.list);
    expect(importSegments(JSON.stringify(previewDefs(segments)))).toEqual(segments);
    segments[0]!.groups[0]!.conditions[0] = { field: "Word Count", op: "greater_or_equal", value: "200" };
    const defs = previewDefs(segments); expect(JSON.stringify(defs[0]?.rules)).toContain('"op":"or"');
    expect(JSON.stringify(defs[0]?.rules)).toContain('"op":"greaterThan"');
    expect(JSON.stringify(defs[1]?.rules)).toContain('"column":"Segment","op":"isEmpty"');
    expect(() => importSegments('[{"name":"bad"}]')).toThrow();
  });
  // spec: CRAWL-134
  it("edits AND/OR groups in the shared draft and only offers earlier segment references", async () => {
    const d = await open("segments"); const blog = within(d).getByRole("group", { name: "Blog" });
    fireEvent.click(within(blog).getByRole("button", { name: "Add OR group" }));
    expect(within(blog).getAllByRole("group")).toHaveLength(2);
    change(within(blog).getAllByLabelText("Operator")[0]!, "in_segment");
    expect(within(blog).queryByRole("option", { name: "Rest" })).toBeNull();
    expect(within(blog).queryByRole("option", { name: "Blog" })).toBeNull();
  });
  // spec: TECH-300
  it("renders 14 ordered elements, defaults to the first 13 and disables Content without both stored crawls", async () => {
    const d = await open("compare.change_detection"); const checkboxes = within(d).getAllByRole("checkbox");
    expect(ELEMENTS).toEqual(changeElements.map(e => e.id)); expect(checkboxes).toHaveLength(14);
    expect(checkboxes.map(c => c.parentElement?.textContent)).toEqual(changeElements.map(e => e.label));
    checkboxes.slice(0, 13).forEach(c => expect(c).toBeChecked()); expect(checkboxes[13]).not.toBeChecked(); expect(checkboxes[13]).toBeDisabled();
    expect(within(d).getByRole("textbox", { name: "Content change threshold (%)" })).toHaveValue("10");
  });
  // spec: TECH-300
  it("enables Content from the core capability and persists ordered selection and threshold", async () => {
    core.handlers.set("compare.contentAvailable", () => true);
    const d = await open("compare.change_detection"); const c = within(d).getByRole("checkbox", { name: "Content" });
    await waitFor(() => expect(c).toBeEnabled()); fireEvent.click(c);
    change(within(d).getByRole("textbox", { name: "Content change threshold (%)" }), "25"); await checked();
    fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    expect(getPath(fake.config, "compare.change_detection")).toEqual({ elements: [...ELEMENTS], content_threshold_pct: 25 });
  });
  // spec: INTEG-020
  // spec: INTEG-040
  // spec: INTEG-060
  it.each([
    ["custom_search", "filters", "Add search filter"],
    ["custom_extraction", "extractors", "Add extractor"],
    ["custom_js", "snippets", "Add snippet"],
  ])("enforces the 100-entry limit for %s without accepting a 101st entry", async (section, list, button) => {
    const template = (getPath(fixture, `${section}.${list}`) as unknown[])[0];
    fake.config[section] = { ...(fake.config[section] as object), [list]: Array.from({ length: 100 }, (_, i) => ({ ...(template as object), name: `Rule ${i + 1}` })) };
    const d = await open(section);
    expect(within(d).getByRole("button", { name: button })).toBeDisabled();
    expect(within(d).getByText("Maximum 100 entries.")).toBeInTheDocument();
  });
  // spec: CRAWL-141
  it("shows all analysis switches and restricts Images to JavaScript rendering", async () => {
    const d = await open("analysis");
    expect(within(d).getAllByRole("switch")).toHaveLength(14);
    expect(within(d).getByRole("switch", { name: "Images" })).toBeDisabled();
    fireEvent.click(within(d).getByRole("switch", { name: "Automatically analyse at crawl end" }));
    await checked(); fireEvent.click(within(d).getByRole("button", { name: "OK" }));
    await waitFor(() => expect(fake.setCalls).toHaveLength(1));
    expect(getPath(fake.config, "analysis.auto_at_end")).toBe(false);
  });
  // spec: INTEG-001
  it("opens the shared API access flow while preserving configuration edits", async () => {
    const { useIntegrations } = await import("../../../integrations/store");
    const d = await open("custom_search");
    change(within(d).getByRole("textbox", { name: "Expression" }), "unsaved expression");
    fireEvent.click(within(d).getByRole("button", { name: "API access" }));
    fireEvent.click(within(d).getByRole("button", { name: "Open API access" }));
    expect(useIntegrations.getState().dialog).not.toBeNull();
    expect(d).toBeInTheDocument(); expect(fake.setCalls).toHaveLength(0);
    fireEvent.click(within(d).getByRole("button", { name: "Custom search" }));
    expect(within(d).getByRole("textbox", { name: "Expression" })).toHaveValue("unsaved expression");
    useIntegrations.setState({ dialog: null });
  });
});

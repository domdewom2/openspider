import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { StructuredData } from "./StructuredData";
import { buildTree, flatten, ownerOf, propertyName, summary, toJsonLd } from "./model";
import { SD_NODES } from "../rendered/fixtures/data";
import { callsOf, lang, selection, settle, setup, teardown } from "../rendered/testing";

vi.mock("../../../app/windows", () => ({ openSettingsWindow: vi.fn().mockResolvedValue(undefined) }));
let ctx: ReturnType<typeof setup>;
const props = () => ({ selection, actions: ctx.actions, rendering: true, htmlStored: true, storeUncrawlable: false, positions: [], listMode: false });
beforeEach(async () => { vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "Date"] }); ctx = setup(); await lang("en"); });
afterEach(async () => { cleanup(); await teardown(); vi.useRealTimers(); });
const issueRows = () => within(screen.getByRole("table", { name: "Validation messages" })).getAllByRole("row").slice(1);
const treeRows = () => within(screen.getByRole("treegrid", { name: "Structured data items" })).getAllByRole("row");
const treeItem = (name: RegExp | string) => treeRows().find(r => typeof name === "string" ? r.textContent?.includes(name) : name.test(r.textContent ?? ""))!;

describe("Structured data details", () => {
  // spec: UI-045
  // spec: TECH-189
  it("builds the tree with formats on top in the order JSON-LD, Microdata, RDFa and properties in source order", () => {
    const tree = buildTree([...SD_NODES].reverse());
    expect(tree.map(e => e.label)).toEqual(["JSON-LD", "Microdata", "RDFa"]);
    const product = tree[0]!.children.find(c => c.label === "Product")!;
    expect(product.children.map(c => `${c.kind}:${c.label}`)).toEqual(["property:name", "item:offers: Offer", "property:brand"]);
    expect(tree[1]!.children[0]!.children.map(c => c.label)).toEqual(["name", "jobTitle"]);
    expect(propertyName("offers[0]")).toBe("offers"); expect(summary(tree)).toEqual({ errors: 2, warnings: 1 });
    expect(toJsonLd(SD_NODES[2]!)).toEqual({ "@context": "https://schema.org", "@type": "Person", name: "Ada", jobTitle: "Engineer" });
    expect(toJsonLd(SD_NODES[0]!)).toMatchObject({ "@type": "Product", offers: { "@type": "Offer", price: "9.99" } });
  });
  // spec: UI-045
  it("shows every finding once and the parse error as its own node with line, column and excerpt", async () => {
    render(<StructuredData {...props()} />); await settle(400);
    expect(issueRows()).toHaveLength(3);
    expect(treeRows().map(r => r.textContent).join("|")).toContain("Parse Error (3:7)");
    fireEvent.click(treeItem("Parse Error")); await settle(0);
    expect(screen.getByText("line 3, column 7")).toBeInTheDocument(); expect(screen.getByText(/"@type": "Pro, "name"/)).toBeInTheDocument();
    expect(issueRows()).toHaveLength(1);
  });
  // spec: TECH-189
  it("filters the findings by tree node, shows the worst symbol on parents and selects the node of a clicked finding", async () => {
    render(<StructuredData {...props()} />); await settle(400);
    const product = treeItem(/^Product/);
    expect(within(product).getByRole("img", { name: "Error" })).toBeInTheDocument();
    fireEvent.click(product); await settle(0); expect(issueRows()).toHaveLength(2);
    fireEvent.click(treeItem("brand")); await settle(0); expect(issueRows()).toHaveLength(1);
    expect(within(issueRows()[0]!).getAllByRole("cell").map(c => c.textContent?.trim())).toEqual(["Schema.org", "Warning", "Property `brand` should be an object.", "brand", "sd.schema.expected_object", "Open"]);
    fireEvent.click(screen.getByRole("button", { name: "Show all findings" })); await settle(0);
    const google = issueRows().find(r => r.textContent?.includes("priceCurrency"))!;
    expect(within(google).getAllByRole("cell")[0]!.textContent).toBe("Google: Merchant listing"); expect(within(google).getAllByRole("cell")[4]!.textContent).toBe("sd.google.missing_required");
    fireEvent.click(google); await settle(0);
    expect(issueRows()).toHaveLength(1); expect(treeRows().find(r => r.getAttribute("aria-selected") === "true")!.textContent).toContain("Offer");
    expect(ownerOf(buildTree(SD_NODES), SD_NODES[0]!.issues[0]!)?.label).toBe("brand");
    expect(flatten(buildTree(SD_NODES)).length).toBeGreaterThan(8);
  });
  // spec: UI-045
  it("copies a Microdata item as JSON-LD from the context menu and refuses it for JSON-LD", async () => {
    render(<StructuredData {...props()} />); await settle(400);
    fireEvent.contextMenu(within(treeItem(/Person/)).getByText("Person")); await settle(0);
    fireEvent.click(screen.getByRole("menuitem", { name: "Copy as JSON-LD" })); await settle(0);
    expect(JSON.parse(vi.mocked(navigator.clipboard.writeText).mock.calls[0]![0])).toEqual({ "@context": "https://schema.org", "@type": "Person", name: "Ada", jobTitle: "Engineer" });
    fireEvent.contextMenu(within(treeItem(/^Product/)).getByText("Product")); await settle(0);
    expect(screen.getByRole("menuitem", { name: "Copy as JSON-LD" })).toHaveAttribute("aria-disabled", "true");
  });
  // spec: UI-045
  // spec: TECH-189
  it("exports the findings through the detail export and opens documentation links in the browser", async () => {
    render(<StructuredData {...props()} />); await settle(400);
    fireEvent.click(screen.getByRole("button", { name: "Export details" }));
    expect(ctx.actions.export).toHaveBeenCalledWith({ kind: "detail", detail: "structuredDataIssues", urlIds: [1], params: {}, sort: [] }, { kind: "ids", ids: [1] });
    fireEvent.click(screen.getAllByRole("button", { name: "Open" })[0]!); expect(ctx.actions.openBrowser).toHaveBeenCalledWith(expect.stringMatching(/^https:\/\//));
    expect(callsOf(ctx.core, "detail.structuredData")).toEqual([{ urlId: 1 }]);
  });
  // spec: TECH-189
  it("is empty with a pointer to the extraction settings when no format is enabled", async () => {
    ctx.options.config = { extraction: { structured_data: { json_ld: false, microdata: false, rdfa: false } } };
    render(<StructuredData {...props()} />); await settle(400);
    expect(document.querySelector('[data-empty-state="FEATURE_DISABLED(extraction.structured_data)"]')).not.toBeNull(); expect(callsOf(ctx.core, "detail.structuredData")).toEqual([]);
  });
});

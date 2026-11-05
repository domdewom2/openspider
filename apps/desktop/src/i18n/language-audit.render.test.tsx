// Sprach-Audit der gerenderten Oberfläche (B3-UI): Tab- und Filternamen kommen als kanonische englische Kernnamen über
// IPC (UI-121, Konvention „Spalten- und Filterschlüssel“). Hat ein Name eine deutsche Übersetzung, darf er in der
// deutschen Oberfläche nicht roh erscheinen. Der Test rendert Seitenleiste (Übersicht, Befunde mit Erklärung) und
// Dashboard mit der Seitenleisten-Fixture und sucht jeden übersetzbaren Kernnamen im Text.
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, expect, it } from "vitest";
import { Providers } from "../app/Providers";
import { Dashboard } from "../features/dashboard/Dashboard";
import { SidebarFrame } from "../features/sidebar/frame/SidebarFrame";
import { useSidebarNavigation } from "../features/sidebar/frame/state";
import { createClient, setClient } from "../ipc/client";
import type { CrawlStatus } from "../ipc/gen/CrawlStatus";
import { MockCore } from "../ipc/transport/mock";
import tick from "../ipc/__fixtures__/Tick.json";
import { useSession } from "../state/session";
import fixture from "../../e2e/sidebar/sidebar-dto.json";
import { filterLabel, tabLabel } from "./canon";
import { makeT, setLanguage } from "./index";

beforeEach(async () => {
  const core = new MockCore();
  core.handlers.set("sidebar.overview", () => fixture.overview);
  core.handlers.set("sidebar.issues", () => fixture.issues);
  core.handlers.set("sidebar.issueExplain", () => fixture.explanation);
  core.handlers.set("analysis.status", () => fixture.analysis);
  setClient(createClient(core.transport()));
  useSession.setState({ seq: -1, crawl: tick.crawl as CrawlStatus, analysis: null, countsVersion: 1, issuesVersion: 1, sidebarVersion: 1, ops: {} });
  useSidebarNavigation.setState({ scope: {}, target: null });
  await setLanguage("de");
});
afterEach(async () => { cleanup(); setClient(null); await setLanguage("en"); });

/** Core names of the fixture whose German label differs from the name itself (otherwise there is nothing to translate). */
function translatableNames(): string[] {
  const de = makeT("de");
  const tabs = [...fixture.overview.tabs.map(t => t.tab), ...fixture.issues.map(i => i.tab)];
  const filters = [...fixture.overview.tabs.flatMap(t => t.filters.map(f => ({ id: f.id, labelTemplate: f.labelTemplate }))), ...fixture.issues.map(i => ({ id: i.filter, labelTemplate: i.filter }))];
  return [...new Set([
    ...tabs.filter(tab => tabLabel(de, tab) !== tab),
    ...filters.filter(f => filterLabel(de, f) !== f.id).map(f => f.id),
  ])];
}

/** Every text node and every element's whole text, trimmed. */
function texts(root: HTMLElement): { nodes: string[]; elements: string[] } {
  const nodes: string[] = [];
  const walker = document.createTreeWalker(root, NodeFilter.SHOW_TEXT);
  for (let n = walker.nextNode(); n; n = walker.nextNode()) if (n.textContent?.trim()) nodes.push(n.textContent.trim());
  const elements = [...root.querySelectorAll<HTMLElement>("button, th, td, h2, h3, span")].map(e => e.textContent?.trim() ?? "");
  return { nodes, elements };
}

// spec: UI-121
// spec: UI-060, UI-061, UI-062
it("shows no translatable English core name in the German sidebar and dashboard (Befunde, Übersicht, Erklärung)", async () => {
  render(<Providers><SidebarFrame /><Dashboard /></Providers>);
  await screen.findByRole("table", { name: "Seitentitel" });
  fireEvent.click(await screen.findByRole("tab", { name: "Befunde" }));
  const issues = await screen.findByRole("tabpanel", { name: "Befunde" });
  const first = await within(issues).findByRole("button", { name: "Seitentitel: Duplikat" });
  await act(async () => { fireEvent.click(first); });
  await screen.findByRole("region", { name: /Erklärung/ });
  const names = translatableNames();
  expect(names).toEqual(expect.arrayContaining(["Page Titles", "Response Codes", "Duplicate", "Missing Alt Attribute"]));
  const { nodes, elements } = texts(document.body);
  const raw = fixture.issues.map(i => `${i.tab}: ${i.filter}`);
  expect(nodes.filter(text => names.includes(text))).toEqual([]);
  expect(elements.filter(text => raw.some(name => text.includes(name)))).toEqual([]);
});

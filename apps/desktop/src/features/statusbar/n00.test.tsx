import { afterEach, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { StatusCard } from "./StatusCard";
import { setupHeader, tick } from "../header/fixtures/setup";
import { useSession } from "../../state/session";
import { setLanguage } from "../../i18n";

afterEach(cleanup);

/** Renders the header status card for a finished crawl built from the running tick fixture. */
async function finished(language: string, endReason: string) {
  setupHeader(); await setLanguage(language);
  useSession.getState().applyTick({ ...tick, seq: 2, crawl: { ...tick.crawl!, phase: "completed", endReason, rate: 0, etaS: null, activeThreads: 0 } });
  render(<StatusCard />);
  const card = screen.getByRole("region", { name: language === "de" ? "Crawl-Fortschritt" : "Crawl progress" });
  return card.querySelector(".os-status-card-state");
}

// spec: UI-003
// spec: CRAWL-126
it.each([
  ["en", "stopped", "Stopped"],
  ["de", "stopped", "Gestoppt"],
  ["en", "queue_empty", "Completed"],
  ["de", "queue_empty", "Fertig"],
])("the header card names the end state of the crawl (%s, %s)", async (language, endReason, text) => {
  // M3-N00 (M2-backlog 5, N02a): after "Stop" the header must not read "Completed"/"Fertig".
  expect(await finished(language, endReason)).toHaveTextContent(new RegExp(`^${text}$`));
});

// spec: UI-013
// spec: CRAWL-130
it.each([
  ["en", "Completed (Limit reached)"],
  ["de", "Fertig (Grenze erreicht)"],
])("the header card shows the limit end like the status bar (%s)", async (language, text) => {
  expect(await finished(language, "limit_reached")).toHaveTextContent(text);
});

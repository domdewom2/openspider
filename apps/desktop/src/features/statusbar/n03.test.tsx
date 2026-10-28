import { afterEach, expect, it } from "vitest";
import { cleanup, render, screen } from "@testing-library/react";
import { Statusbar } from "./Statusbar";
import { setupHeader, tick } from "../header/fixtures/setup";
import { useSession } from "../../state/session";
import { setLanguage } from "../../i18n";

afterEach(cleanup);
// spec: CRAWL-145
// spec: UI-013
it.each([false, true])("keeps the Crawl-delay explanation visible after notices are consumed (applied=%s)", async applied => {
  setupHeader(); await setLanguage("en");
  useSession.setState({ crawlDelays: {} });
  useSession.getState().applyTick({ ...tick, notices: [{ level: "info", code: "crawl-delay", args: { origin: "https://site.test", seconds: 1, applied } }] });
  useSession.getState().clearNotices();
  render(<Statusbar />);
  expect(screen.getByText(applied ? /Rate limit: Crawl-delay 1.0 s.*applied/ : /Crawl-delay 1.0 s.*not applied/)).toBeVisible();
});

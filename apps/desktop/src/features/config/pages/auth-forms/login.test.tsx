import { act, cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, expect, it } from "vitest";
import { setLanguage } from "../../../../i18n";
import { createClient, setClient } from "../../../../ipc/client";
import { MockCore } from "../../../../ipc/transport/mock";
import { useSession } from "../../../../state/session";
import { ConfigHost } from "../../frame/ConfigHost";
import { installConfigCore } from "../../frame/testing";
import { openConfig } from "../../frame/store";
import { Header } from "../../../header/Header";
import { Statusbar } from "../../../statusbar/Statusbar";
import { setupHeader } from "../../../header/fixtures/setup";
import { guardDefaults } from "./session";

afterEach(() => { cleanup(); setClient(null); });
// spec: CRAWL-120
it("captures metadata into the draft, populates guard rules, and cancels a pending browser", async () => {
  const core = new MockCore(); installConfigCore(core);
  setClient(createClient(core.transport("main"))); await setLanguage("en");
  useSession.setState({ crawl: null, ops: {}, notices: [] });
  let saved = false;
  core.handlers.set("auth.loginStatus", () => saved ? [{ profile: "form-profile", loginUrl: "https://fixture.test/login", state: "active", cookies: 2, capturedAt: "2026-10-27T00:00:00Z" }] : []);
  core.handlers.set("auth.openLogin", () => ({ opId: "login-op" }));
  core.handlers.set("auth.finishLogin", () => { saved = true; return { name: "form-profile", kind: "form", host: "fixture.test", createdAt: "2026-10-27T00:00:00Z", hasSecret: true }; });
  core.handlers.set("op.cancel", () => undefined);
  render(<ConfigHost />); act(() => openConfig({ anchor: "auth.form" }));
  const dialog = await screen.findByRole("dialog", { name: "Crawl configuration" });
  fireEvent.change(within(dialog).getByRole("textbox", { name: "Login URL" }), { target: { value: "https://FIXTURE.test/login" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Add and open browser" }));
  const login = await screen.findByRole("dialog", { name: "Form authentication" });
  fireEvent.click(within(login).getByRole("button", { name: "Capture session" }));
  await screen.findByText("Signed in, 2 cookies");
  expect((within(dialog).getByLabelText("Session guard patterns") as HTMLTextAreaElement).value).toBe(guardDefaults.join("\n"));
  expect(JSON.stringify(core.calls)).not.toContain("COOKIE-FIXTURE");
  fireEvent.change(within(dialog).getByRole("textbox", { name: "Login URL" }), { target: { value: "https://fixture.test/next" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "Add and open browser" }));
  const next = await screen.findByRole("dialog", { name: "Form authentication" });
  fireEvent.click(within(next).getByRole("button", { name: "Cancel" }));
  await waitFor(() => expect(core.calls.filter((c) => c.cmd === "op.cancel")).toHaveLength(1));
});
// spec: CRAWL-120
it("requires explicit confirmation before starting with an active form session and empty guard list", async () => {
  const core = setupHeader(); await setLanguage("en");
  core.handlers.set("config.get", () => ({ schemaVersion: 2, generator: "fixture", config: { auth: { form: [{ profile: "login" }], session_guard: [] } } }));
  render(<Header />);
  fireEvent.change(screen.getByLabelText("Start URL"), { target: { value: "https://fixture.test/" } });
  fireEvent.click(screen.getByRole("button", { name: "Start" }));
  const warning = await screen.findByRole("dialog", { name: "Start without session protection?" });
  expect(core.calls.filter((c) => c.cmd === "crawl.start")).toHaveLength(0);
  fireEvent.click(within(warning).getByRole("button", { name: "Cancel" }));
  fireEvent.click(screen.getByRole("button", { name: "Start" }));
  const confirmed = await screen.findByRole("dialog", { name: "Start without session protection?" });
  fireEvent.click(within(confirmed).getByRole("button", { name: "Start anyway" }));
  await waitFor(() => expect(core.calls.filter((c) => c.cmd === "crawl.start")).toHaveLength(1));
});
// spec: CRAWL-120
it("shows a persisted session-loss warning in the status bar", async () => {
  const core = setupHeader(); await setLanguage("en");
  core.handlers.set("auth.loginStatus", () => [{ loginUrl: "https://fixture.test/login", state: "lost", cookies: 1, capturedAt: null }]);
  const { tick } = await import("../../../header/fixtures/setup");
  useSession.getState().applyTick(tick);
  render(<Statusbar />);
  expect(await screen.findByText("Session lost")).toBeVisible();
});

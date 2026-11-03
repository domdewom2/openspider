// OUT-246/OUT-247: Hinweis beim Start, Ansehen (genau der Text), Senden nur nach Klick, Fehler mit „Erneut versuchen“,
// Hilfe → „Absturzberichte…“, Richtlinie, Einstellung und Fehlergrenzen (de/en).
import { act, cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useState } from "react";
import { setLanguage } from "../../i18n";
import { createClient, setClient } from "../../ipc/client";
import type { CrashOverview } from "../../ipc/gen/CrashOverview";
import type { CrashSendResult } from "../../ipc/gen/CrashSendResult";
import type { CrashSendArgs } from "../../ipc/gen/CrashSendArgs";
import { MockCore } from "../../ipc/transport/mock";
import { runAction } from "../../ui/actions";
import { RegionBoundary, RootBoundary, useCrashProbe } from "../../app/ErrorBoundary";
import { CrashHost, failureText, validEmail } from "./CrashHost";
import { resetCrashState, useCrash } from "./state";
import { getT } from "../../i18n";

const REPORT = '{\n  "format": 1,\n  "id": "a",\n  "kind": "panic"\n}';
let core: MockCore;
let overview: CrashOverview;
let sendReply: (args: CrashSendArgs) => CrashSendResult;

function summary(id: string, extra: Partial<CrashOverview["reports"][number]> = {}) {
  return { id, kind: "panic", time: "2026-11-02T10:15:00.123+01:00", process: "app", message: "boom", sent: null, serverRef: null, ...extra };
}
const calls = (cmd: string) => core.calls.filter((c) => c.cmd === cmd);

beforeEach(async () => {
  core = new MockCore();
  overview = { reports: [summary("a"), summary("b", { kind: "unclean_exit" })], pending: ["a", "b"], promptEnabled: true, disabledByPolicy: false };
  sendReply = (args) => ({ results: args.ids.map((id) => ({ id, serverRef: "ABCD2345", failure: null })) });
  core.handlers.set("crash.overview", () => structuredClone(overview));
  core.handlers.set("crash.read", (args) => (args as { id: string }).id === "a" ? REPORT : REPORT.replace('"a"', '"b"'));
  core.handlers.set("crash.send", (args) => {
    const a = args as CrashSendArgs;
    const reply = sendReply(a);
    for (const r of reply.results) if (r.serverRef) overview.reports = overview.reports.map((s) => s.id === r.id ? { ...s, sent: "2026-11-03T10:00:00+01:00", serverRef: r.serverRef } : s);
    overview.pending = overview.pending.filter((id) => !reply.results.some((r) => r.id === id && r.serverRef));
    return reply;
  });
  core.handlers.set("crash.dismiss", () => { overview.pending = []; return null; });
  core.handlers.set("crash.delete", (args) => { overview.reports = overview.reports.filter((r) => r.id !== (args as { id: string }).id); return null; });
  core.handlers.set("crash.reportFrontend", () => true);
  setClient(createClient(core.transport("main")));
  resetCrashState();
  await setLanguage("en");
});
afterEach(async () => { cleanup(); resetCrashState(); await setLanguage("en"); });

async function host() {
  render(<CrashHost />);
  await act(async () => {});
}

describe("start hint", () => {
  // spec: OUT-247
  for (const [lang, title, view, send, dont] of [
    ["de", "OpenSpider hatte beim letzten Lauf ein Problem.", "Bericht ansehen…", "Senden", "Nicht senden"],
    ["en", "OpenSpider ran into a problem last time.", "View report…", "Send", "Don't send"],
  ] as const) {
    it(`${lang}: shows the non-modal hint with view, send and don't send, without sending anything`, async () => {
      await setLanguage(lang);
      await host();
      const hint = screen.getByRole("region", { name: lang === "de" ? "Absturzbericht" : "Crash report" });
      expect(within(hint).getByText(title)).toBeTruthy();
      for (const name of [view, send, dont]) expect(within(hint).getByRole("button", { name })).toBeTruthy();
      expect(screen.queryByRole("dialog")).toBeNull();
      expect(calls("crash.send")).toHaveLength(0);
      expect(calls("crash.read")).toHaveLength(0);
    });
  }

  // spec: OUT-247
  it("does not appear without pending reports, with the setting off or under policy", async () => {
    for (const change of [{ pending: [] }, { pending: [], promptEnabled: false }, { pending: [], disabledByPolicy: true }]) {
      overview = { ...overview, ...change };
      resetCrashState();
      await host();
      expect(screen.queryByRole("region", { name: "Crash report" })).toBeNull();
      cleanup();
    }
  });

  // spec: OUT-247
  it("view shows exactly the JSON text that is sent; note and email go along", async () => {
    await host();
    fireEvent.click(screen.getByRole("button", { name: "View report…" }));
    const dialog = await screen.findByRole("dialog", { name: "Crash report" });
    await act(async () => {});
    const json = within(dialog).getByLabelText("Report content (sent exactly like this)") as HTMLTextAreaElement;
    expect(json.value).toBe(REPORT);
    expect(json.readOnly).toBe(true);
    // zwei Berichte im Hinweis: Auswahl wechselt den Text
    fireEvent.change(within(dialog).getByLabelText("Report"), { target: { value: "b" } });
    await act(async () => {});
    expect((within(dialog).getByLabelText("Report content (sent exactly like this)") as HTMLTextAreaElement).value).toContain('"b"');
    expect(calls("crash.send")).toHaveLength(0);
    fireEvent.change(within(dialog).getByLabelText("What were you doing last? (optional)"), { target: { value: "Export" } });
    const email = within(dialog).getByLabelText("Email for follow-up questions (optional)");
    fireEvent.change(email, { target: { value: "kein@" } });
    expect(within(dialog).getByRole("alert").textContent).toContain("valid email");
    expect((within(dialog).getByRole("button", { name: "Send" }) as HTMLButtonElement).disabled).toBe(true);
    fireEvent.change(email, { target: { value: "ops@example.com" } });
    await act(async () => { fireEvent.click(within(dialog).getByRole("button", { name: "Send" })); });
    expect(calls("crash.send").map((c) => c.args)).toEqual([{ ids: ["a", "b"], note: "Export", email: "ops@example.com" }]);
    expect(screen.getAllByTestId("crash-thanks")[0]!.textContent).toBe("Thank you! Reference: ABCD2345, ABCD2345");
  });

  // spec: OUT-247
  it("don't send hides the hint permanently via crash.dismiss and sends nothing", async () => {
    await host();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Don't send" })); });
    expect(calls("crash.dismiss").map((c) => c.args)).toEqual([{ ids: ["a", "b"] }]);
    expect(screen.queryByRole("region", { name: "Crash report" })).toBeNull();
    expect(calls("crash.send")).toHaveLength(0);
  });

  // spec: OUT-247
  for (const [failure, text] of [
    [{ http: 500 }, "Sending failed: The server answered with HTTP 500."],
    [{ http: 429 }, "Sending failed: Too many reports in a short time (HTTP 429). Please try again later."],
    ["timeout", "Sending failed: Timed out (no answer within 15 seconds)."],
    ["network", "Sending failed: No connection to the server."],
  ] as const) {
    it(`failure ${JSON.stringify(failure)} shows the cause and try again sends again`, async () => {
      sendReply = (args) => ({ results: [{ id: args.ids[0]!, serverRef: null, failure }] });
      await host();
      const hint = screen.getByRole("region", { name: "Crash report" });
      await act(async () => { fireEvent.click(within(hint).getByRole("button", { name: "Send" })); });
      expect(within(hint).getByRole("alert").textContent).toBe(text);
      sendReply = (args) => ({ results: args.ids.map((id) => ({ id, serverRef: "WXYZ6789", failure: null })) });
      await act(async () => { fireEvent.click(within(hint).getByRole("button", { name: "Try again" })); });
      expect(calls("crash.send")).toHaveLength(2);
      expect(within(hint).getByTestId("crash-thanks").textContent).toBe("Thank you! Reference: WXYZ6789, WXYZ6789");
    });
  }

  // spec: OUT-247
  it("de: thanks with reference", async () => {
    await setLanguage("de");
    await host();
    await act(async () => { fireEvent.click(screen.getByRole("button", { name: "Senden" })); });
    expect(screen.getByTestId("crash-thanks").textContent).toBe("Danke! Referenz: ABCD2345, ABCD2345");
  });
});

describe("Help → Crash reports…", () => {
  // spec: OUT-247
  // spec: UI-002 [M3]
  it("lists time, type, sent and reference with view, send and delete", async () => {
    overview.reports = [summary("a"), summary("c", { kind: "frontend", sent: "2026-11-02T11:00:00+01:00", serverRef: "QRST2345" })];
    overview.pending = [];
    await host();
    act(() => runAction("help.crashReports", { source: "menu" }));
    const dialog = await screen.findByRole("dialog", { name: "Crash reports" });
    await act(async () => {});
    const rows = within(dialog).getAllByRole("row");
    expect(rows).toHaveLength(3);
    expect(within(rows[1]!).getByText("Program error")).toBeTruthy();
    expect(within(rows[1]!).getByText("no")).toBeTruthy();
    expect(within(rows[2]!).getByText("Interface error")).toBeTruthy();
    expect(within(rows[2]!).getByText("yes")).toBeTruthy();
    expect(within(rows[2]!).getByText("QRST2345")).toBeTruthy();
    expect((within(rows[2]!).getByRole("button", { name: "Send" }) as HTMLButtonElement).disabled).toBe(true);
    await act(async () => { fireEvent.click(within(rows[1]!).getByRole("button", { name: "Send" })); });
    expect(calls("crash.send").map((c) => c.args)).toEqual([{ ids: ["a"], note: "", email: "" }]);
    await act(async () => { fireEvent.click(within(within(dialog).getAllByRole("row")[2]!).getByRole("button", { name: "Delete" })); });
    expect(calls("crash.delete").map((c) => c.args)).toEqual([{ id: "c" }]);
    expect(within(dialog).getAllByRole("row")).toHaveLength(2);
  });

  // spec: OUT-247
  it("policy: send is disabled with 'disabled by your administrator' and no request is made", async () => {
    overview = { ...overview, pending: [], disabledByPolicy: true };
    await host();
    act(() => runAction("help.crashReports", { source: "menu" }));
    const dialog = await screen.findByRole("dialog", { name: "Crash reports" });
    await act(async () => {});
    expect(within(dialog).getByText(/disabled by your administrator/)).toBeTruthy();
    for (const button of within(dialog).getAllByRole("button", { name: "Send" })) expect((button as HTMLButtonElement).disabled).toBe(true);
    fireEvent.click(within(dialog).getAllByRole("button", { name: "View" })[0]!);
    const view = await screen.findByRole("dialog", { name: "Crash report" });
    expect((within(view).getByRole("button", { name: "Send" }) as HTMLButtonElement).disabled).toBe(true);
    expect(calls("crash.send")).toHaveLength(0);
  });
});

describe("helpers", () => {
  // spec: OUT-247
  it("email validation and failure texts", async () => {
    expect(validEmail("")).toBe(true);
    expect(validEmail("a@b.de")).toBe(true);
    expect(validEmail("a@b")).toBe(false);
    expect(validEmail(`${"a".repeat(250)}@b.de`)).toBe(false);
    await setLanguage("de");
    const t = getT();
    expect(failureText(t, "disabled")).toBe("Durch Administrator deaktiviert.");
    expect(failureText(t, { http: 413 })).toBe("Der Server antwortete mit HTTP 413.");
    expect(failureText(t, "notFound")).toBe("Der Bericht wurde nicht gefunden.");
    expect(failureText(t, "alreadySent")).toBe("Der Bericht wurde bereits gesendet.");
  });
});

function Exploding({ when }: { when: boolean }) {
  if (when) throw new Error("boom in table");
  return <p>table content</p>;
}

describe("error boundaries", () => {
  // spec: OUT-246
  it("a region failure reports exactly one frontend crash with message and component stack", async () => {
    const quiet = console.error;
    console.error = () => undefined;
    try {
      function Harness() {
        const [boom, setBoom] = useState(false);
        return <><button onClick={() => setBoom(true)}>explode</button><RegionBoundary name="table"><Exploding when={boom} /></RegionBoundary></>;
      }
      render(<Harness />);
      expect(screen.getByText("table content")).toBeTruthy();
      await act(async () => { fireEvent.click(screen.getByText("explode")); });
      expect(document.querySelector('[data-region-failed="table"]')).not.toBeNull();
    } finally {
      console.error = quiet;
    }
    const reports = calls("crash.reportFrontend").map((c) => c.args as { message: string; componentStack: string; region: string });
    expect(reports).toHaveLength(1);
    expect(reports[0]!.message).toBe("boom in table");
    expect(reports[0]!.region).toBe("table");
    expect(reports[0]!.componentStack).toContain("Exploding");
    expect(calls("crash.send")).toHaveLength(0);
  });

  // spec: OUT-246
  it("the whole view failing (root boundary) and the dev test probe report too", async () => {
    const quiet = console.error;
    console.error = () => undefined;
    try {
      render(<RootBoundary><RegionBoundary name="detail"><p>ok</p></RegionBoundary></RootBoundary>);
      await act(async () => { useCrashProbe.setState({ region: "detail" }); });
      await act(async () => { useCrashProbe.setState({ region: "root" }); });
    } finally {
      console.error = quiet;
    }
    const regions = calls("crash.reportFrontend").map((c) => (c.args as { region: string }).region);
    expect(regions).toEqual(["detail", "root"]);
    expect(useCrashProbe.getState().region).toBeNull();
    expect(useCrash.getState().hintOpen).toBe(false);
  });
});

describe("setting crash_reports.prompt", () => {
  // spec: OUT-247
  it("defaults on, persists crashReports.prompt and is greyed out under policy", async () => {
    const { usePrefs } = await import("../../state/prefs");
    const { CrashPromptSetting } = await import("./CrashPromptSetting");
    await act(async () => { await usePrefs.getState().load(); });
    render(<CrashPromptSetting />);
    await act(async () => {});
    const toggle = screen.getByRole("switch", { name: "After a crash, ask whether to send a report" }) as HTMLInputElement;
    expect(toggle.checked).toBe(true);
    await act(async () => { fireEvent.click(toggle); });
    await act(async () => {});
    expect(calls("settings.set").at(-1)?.args).toEqual({ patch: { crashReports: { prompt: false } } });
    expect(usePrefs.getState().dto?.crashReports.prompt).toBe(false);
    cleanup();
    overview = { ...overview, disabledByPolicy: true };
    render(<CrashPromptSetting />);
    await act(async () => {});
    const locked = screen.getByRole("switch", { name: "After a crash, ask whether to send a report" }) as HTMLInputElement;
    expect(locked.disabled).toBe(true);
    expect(screen.getByText(/disabled by your administrator/)).toBeTruthy();
  });
});

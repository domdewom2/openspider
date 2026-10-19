import { afterEach, beforeEach, expect, it } from "vitest";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { createClient, setClient } from "../../../../ipc/client";
import { setLanguage } from "../../../../i18n";
import { useSession } from "../../../../state/session";
import { taskFixture } from "../../../tasks/fixtures/core";
import type { MailAccountInput } from "../../../../ipc/gen/MailAccountInput";
import { EmailSettings } from "./index";

let fx: ReturnType<typeof taskFixture>;
beforeEach(async () => { fx = taskFixture(); setClient(createClient(fx.core.transport("settings"))); useSession.setState({ ops: {} }); await setLanguage("en"); });
afterEach(() => { cleanup(); setClient(null); });
const click = (name: string) => fireEvent.click(screen.getByRole("button", { name }));
const fill = (label: string, value: string) => fireEvent.change(screen.getByLabelText(label), { target: { value } });

// spec: OUT-280 [P1]
it("requires explicit confirmation for plaintext SMTP, sends secrets only to IPC, and clears the form", async () => {
  render(<EmailSettings />); await screen.findByText(/Work SMTP/); click("Add SMTP account");
  fill("Account label", "Local test"); fill("SMTP host", "127.0.0.1"); fill("SMTP port", "2525"); fill("Username", "tester"); fill("Password", "fixture-password"); fill("Sender address", "test@example.test");
  fireEvent.change(screen.getByLabelText("Encryption"), { target: { value: "none" } });
  expect(screen.getByRole("button", { name: "Save account" })).toBeDisabled();
  expect(fx.core.calls.some(c => c.cmd === "mail.saveAccount")).toBe(false);
  fireEvent.click(screen.getByRole("checkbox", { name: /I confirm sending credentials/ }));
  click("Save account"); await waitFor(() => expect(screen.queryByLabelText("Password")).not.toBeInTheDocument());
  expect(fx.core.calls.find(c => c.cmd === "mail.saveAccount")?.args).toEqual({ account: { id: null, label: "Local test", host: "127.0.0.1", port: 2525, tls: "none", username: "tester", password: "fixture-password", from: "test@example.test" } });
  expect(document.body.textContent).not.toContain("fixture-password");
  expect(JSON.stringify(localStorage)).not.toContain("fixture-password");
});

// spec: OUT-280 [P1]
it("keeps the stored password on edit and never populates it in the input", async () => {
  render(<EmailSettings />); await screen.findByText(/Work SMTP/); click("Edit SMTP account");
  expect(screen.getByLabelText("Password")).toHaveValue(""); fill("Username", "tester"); click("Save account");
  await waitFor(() => expect(fx.core.calls.some(c => c.cmd === "mail.saveAccount")).toBe(true));
  expect((fx.core.calls.find(c => c.cmd === "mail.saveAccount")?.args as { account: MailAccountInput }).account.password).toBeNull();
});

// spec: OUT-280 [P1]
it("displays async SMTP success and a readable failure including its reply code", async () => {
  render(<EmailSettings />); await screen.findByText(/Work SMTP/); fill("Test email recipient", "receiver@example.test"); click("Send test email");
  await screen.findByText("Sending test email…");
  act(() => useSession.setState({ ops: { "operation-1": { opId: "operation-1", labelKey: "email-test", done: 1, state: "done", result: { ok: false, error: { code: "Permission", messageKey: "email-test-failed", args: { smtpCode: 535 }, retryable: false } } } } }));
  expect(await screen.findByRole("alert")).toHaveTextContent("Test email failed. (SMTP 535)");
  click("Send test email");
  await waitFor(() => expect(fx.core.calls.filter(c => c.cmd === "mail.testAccount")).toHaveLength(2));
  act(() => useSession.setState({ ops: { "operation-2": { opId: "operation-2", labelKey: "email-test", done: 1, state: "done", result: { ok: true } } } }));
  expect(await screen.findByText("Test email sent.")).toBeInTheDocument();
});

// spec: OUT-280 [P1]
it("does not call delete until the user confirms", async () => {
  render(<EmailSettings />); await screen.findByText(/Work SMTP/); click("Delete account");
  expect(fx.core.calls.some(c => c.cmd === "mail.deleteAccount")).toBe(false);
  const dialogs = screen.getAllByRole("button", { name: "Delete account" }); fireEvent.click(dialogs[dialogs.length - 1]);
  await waitFor(() => expect(fx.core.calls.find(c => c.cmd === "mail.deleteAccount")?.args).toEqual({ id: "work" }));
});

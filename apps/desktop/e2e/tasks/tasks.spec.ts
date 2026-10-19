import { test, expect } from "@playwright/test";
import type {} from "./harness";

test.beforeEach(async ({ page }) => { await page.goto("/e2e/tasks/index.html"); await page.waitForFunction(() => !!window.taskFixture); });

// spec: OUT-160
// spec: OUT-161 [P1]
test("task editor tabs support keyboard navigation and persist edits", async ({ page }) => {
  await page.getByRole("button", { name: "Scheduling", exact: true }).click();
  await page.getByRole("checkbox", { name: "Select Nightly" }).check();
  await page.getByRole("button", { name: "Edit task", exact: true }).click();
  const editor = page.getByRole("dialog", { name: "Edit task", exact: true });
  await editor.getByLabel("Task name", { exact: true }).fill("Nightly edited");
  const tabs = editor.getByRole("tablist", { name: "Task settings", exact: true });
  await expect(tabs.getByRole("tab")).toHaveText(["General", "Spider", "APIs", "Exports"]);
  await tabs.getByRole("tab", { name: "General", exact: true }).focus();
  await page.keyboard.press("ArrowRight");
  await expect(tabs.getByRole("tab", { name: "Spider", exact: true })).toBeFocused();
  await page.keyboard.press("Enter");
  await tabs.getByRole("tab", { name: "Exports", exact: true }).click();
  await expect(editor.getByRole("tablist", { name: "Export settings", exact: true }).getByRole("tab")).toHaveText(["Output settings", "Tables", "Local files", "Looker Studio", "Templates / presets", "Notifications"]);
  await editor.getByRole("button", { name: "Save", exact: true }).click();
  await expect(page.getByRole("cell", { name: "Nightly edited", exact: true })).toBeVisible();
  const saves = await page.evaluate(() => window.taskFixture.calls().filter(c => c.cmd === "tasks.save"));
  expect(saves).toHaveLength(1);
});

// spec: OUT-210 [M2]
// spec: OUT-211 [M2]
test("Custom Crawl Overview preserves a canonical ordered metric list", async ({ page }) => {
  await page.getByRole("button", { name: "Scheduling", exact: true }).click();
  await page.getByRole("checkbox", { name: "Select Nightly" }).check();
  await page.getByRole("button", { name: "Edit task", exact: true }).click();
  await page.getByRole("tab", { name: "Exports", exact: true }).click();
  await page.getByRole("tab", { name: "Looker Studio", exact: true }).click();
  await page.getByText("Custom Crawl Overview", { exact: true }).click();
  await page.getByRole("button", { name: "Internal:HTML", exact: true }).click();
  await page.getByRole("button", { name: "Response Codes:Client Error (4xx)", exact: true }).click();
  await page.getByRole("button", { name: "Move Response Codes:Client Error (4xx) up", exact: true }).click();
  await page.getByRole("button", { name: "Show selection as text", exact: true }).click();
  await expect(page.getByLabel(/Metric names/)).toHaveValue("Response Codes:Client Error (4xx)\nInternal:HTML");
});

// spec: OUT-169 [P1]
// spec: OUT-171 [P1]
test("run history and abort are available without a pause action", async ({ page }) => {
  await page.getByRole("button", { name: "Scheduling", exact: true }).click();
  await page.getByRole("checkbox", { name: "Select Nightly" }).check();
  await page.getByRole("button", { name: "Run now", exact: true }).click();
  await page.getByRole("button", { name: "Abort run", exact: true }).click();
  expect(await page.evaluate(() => window.taskFixture.calls().some(c => c.cmd === "op.cancel"))).toBe(true);
  await page.getByRole("button", { name: "Run history", exact: true }).click();
  await expect(page.getByRole("cell", { name: "Completed with warnings", exact: true })).toBeVisible();
  await expect(page.getByRole("cell", { name: "Aborted", exact: true })).toBeVisible();
  await expect(page.getByRole("button", { name: /pause/i })).toHaveCount(0);
});

// spec: OUT-280 [P1]
test("plaintext SMTP cannot be saved without explicit confirmation", async ({ page }) => {
  await page.getByRole("button", { name: "Add SMTP account", exact: true }).click();
  await page.getByLabel("Account label", { exact: true }).fill("Test SMTP");
  await page.getByLabel("SMTP host", { exact: true }).fill("smtp.example.test");
  await page.getByLabel("Username", { exact: true }).fill("test");
  await page.getByLabel("Sender address", { exact: true }).fill("test@example.test");
  await page.getByRole("combobox", { name: "Encryption", exact: true }).selectOption("none");
  await expect(page.getByRole("button", { name: "Save account", exact: true })).toBeDisabled();
  await page.getByText(/I confirm sending credentials/).click();
  await expect(page.getByRole("button", { name: "Save account", exact: true })).toBeEnabled();
});

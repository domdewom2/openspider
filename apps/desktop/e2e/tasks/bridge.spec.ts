import { test, expect } from "@playwright/test";
import { startBridge } from "../support/bridge";
import type { TaskDef } from "../../src/ipc/gen/TaskDef";
import fixture from "../../src/features/tasks/fixtures/out-schedule.json" with { type: "json" };
import type {} from "./harness";

// spec: OUT-160
// spec: OUT-161 [P1]
test("real bridge saves and reloads a scheduled task", async ({ page }) => {
  const bridge = await startBridge();
  expect(bridge, "real bridge required").not.toBeNull();
  try {
    await page.goto(`/e2e/tasks/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.taskFixture);
    const task: TaskDef = { ...fixture.task as TaskDef, id: null, name: "UI bridge task", enabled: false, crawl: { ...(fixture.task as TaskDef).crawl, profile: null, authProfile: null }, apis: [], exports: [], notify: { onSuccess: false, onFailure: false, to: [] } };
    const saved = await page.evaluate(async json => window.taskFixture.invoke("tasks.save", { task: JSON.parse(json) }), JSON.stringify(task)) as { id: string };
    try {
      await page.getByRole("button", { name: "Scheduling", exact: true }).click();
      await expect(page.getByRole("cell", { name: task.name, exact: true })).toBeVisible();
      const loaded = await page.evaluate(id => window.taskFixture.invoke("tasks.get", { id }), saved.id) as TaskDef;
      expect(loaded.schedule).toEqual(task.schedule);
    } finally { await page.evaluate(id => window.taskFixture.invoke("tasks.delete", { id }), saved.id); }
  } finally { await bridge?.stop(); }
});

// spec: OUT-210 [M2]
// spec: OUT-211 [M2]
test("real registry populates the custom summary dialog", async ({ page }) => {
  const bridge = await startBridge();
  expect(bridge, "real bridge required").not.toBeNull();
  try {
    await page.goto(`/e2e/tasks/index.html?bridge=${encodeURIComponent(bridge!.url)}`);
    await page.waitForFunction(() => !!window.taskFixture);
    await page.getByRole("button", { name: "Scheduling", exact: true }).click();
    await page.getByRole("button", { name: "Add task", exact: true }).click();
    await page.getByRole("tab", { name: "Exports", exact: true }).click();
    await page.getByRole("tab", { name: "Looker Studio", exact: true }).click();
    await page.getByText("Custom Crawl Overview", { exact: true }).click();
    await expect(page.getByRole("button", { name: "Internal:HTML", exact: true })).toBeVisible();
  } finally { await bridge?.stop(); }
});

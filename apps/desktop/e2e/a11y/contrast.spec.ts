import { shortcut, test, expect, BARS, tabTo, visibleFocus, selectFirstRow, scan } from "./fixture";

test.use({ locale: "en-US" });
test.setTimeout(240_000);
for (const mode of ["forced-colors", "more"] as const) {
  for (const theme of ["light", "dark"] as const) {
    // spec: UI-126
    test(`${mode}, ${theme}: controls, focus and selected tabs remain distinguishable`, async ({ page, audit, context }) => {
      const media = { colorScheme: theme, ...(mode === "forced-colors" ? { forcedColors: "active" as const } : { contrast: "more" as const }) };
      await page.emulateMedia(media);
      expect(await page.evaluate(mode => matchMedia(mode === "forced-colors" ? "(forced-colors: active)" : "(prefers-contrast: more)").matches, mode)).toBe(true);
      await audit.crawl();
      await selectFirstRow(page);
      await visibleFocus(page.locator("main .os-data-grid"));
      for (const name of BARS) {
        const list = page.getByRole("tablist", { name, exact: true });
        const selected = list.getByRole("tab", { selected: true });
        await tabTo(page, selected);
        await visibleFocus(selected);
        const style = await selected.evaluate(el => {
          const s = getComputedStyle(el);
          return { fg: s.color, bg: s.backgroundColor, weight: Number(s.fontWeight), border: s.borderBottomStyle, borderWidth: parseFloat(s.borderBottomWidth), shadow: s.boxShadow };
        });
        expect(style.fg).not.toBe(style.bg);
        // A non-colour selected cue must survive system colour replacement.
        expect(style.weight >= 600 || style.border !== "none" && style.borderWidth > 0 || style.shadow !== "none").toBe(true);
        const trigger = list.locator("..").getByRole("button");
        await tabTo(page, trigger);
        await visibleFocus(trigger);
        await trigger.press("Enter");
        await expect(page.getByRole("menu")).toBeVisible();
        await scan(page, `${mode}-${theme}-${name}-overflow`);
        await page.keyboard.press("Escape");
        await expect(trigger).toBeFocused();
      }
      await scan(page, `${mode}-${theme}-main`);
      await tabTo(page, page.locator("main .os-data-grid"));
    await shortcut(page, "e");
      const dialog = page.getByRole("dialog", { name: "Export options", exact: true });
      await expect(dialog).toBeVisible();
      const button = dialog.getByRole("button", { name: "Cancel", exact: true });
      await tabTo(page, button);
      await visibleFocus(button);
      await scan(page, `${mode}-${theme}-export`);
      await page.keyboard.press("Escape");
      const settings = await context.newPage();
      await settings.emulateMedia(media);
      await settings.goto(`/settings.html?bridge=${encodeURIComponent(audit.bridge)}`);
      const appearance = settings.getByRole("tab", { name: "Appearance", exact: true });
      await tabTo(settings, settings.getByRole("tab", { selected: true }));
      await settings.keyboard.press("End");
      await appearance.press("Enter");
      await expect(appearance).toHaveAttribute("aria-selected", "true");
      await scan(settings, `${mode}-${theme}-settings`);
    });
  }
}

for (const mode of ["forced-colors", "more"] as const) {
  for (const theme of ["light", "dark"] as const) {
    // spec: UI-126
    test(`${mode}, ${theme}: primitive checkbox, switch, radio and colour selection have visible state cues`, async ({ page }) => {
      await page.emulateMedia({ colorScheme: theme, ...(mode === "forced-colors" ? { forcedColors: "active" as const } : { contrast: "more" as const }) });
      await page.goto(`/?harness=primitives&theme=${theme}&lang=en`);
      const checkbox = page.getByRole("checkbox");
      await expect(checkbox).toBeChecked();
      await tabTo(page, checkbox);
      await page.keyboard.press("Space");
      await expect(checkbox).not.toBeChecked();
      const mark = await page.locator(".os-checkbox-box").evaluate(el => {
        const svg = el.querySelector("svg");
        if (!svg) return false;
        const s = getComputedStyle(svg);
        return s.opacity !== "0" && s.visibility !== "hidden" && s.color !== "rgba(0, 0, 0, 0)";
      });
      expect(mark, "unchecked checkbox must not show the check mark after system colour replacement").toBe(false);
      const track = page.locator(".os-switch-track");
      const thumb = page.locator(".os-switch-thumb");
      expect(await thumb.evaluate(el => getComputedStyle(el).backgroundColor)).not.toBe(await track.evaluate(el => getComputedStyle(el).backgroundColor));
      const radio = page.getByRole("radio", { name: "Light", exact: true });
      await tabTo(page, radio);
      await page.keyboard.press("ArrowRight");
      await expect(page.getByRole("radio", { name: "Dark", exact: true })).toBeChecked();
      const green = page.getByRole("button", { name: "Green", exact: true });
      await tabTo(page, green);
      await page.keyboard.press("Space");
      await expect(green).toHaveAttribute("aria-pressed", "true");
      await page.keyboard.press("Tab");
      if (mode === "forced-colors") {
        expect(await green.evaluate(el => parseFloat(getComputedStyle(el).outlineWidth))).toBeGreaterThan(0);
      }
      await scan(page, `${mode}-${theme}-primitives`);
    });
  }
}

const FOCUSABLE = 'input:not([disabled]),textarea:not([disabled]),select:not([disabled]),button:not([disabled]),[tabindex]:not([tabindex="-1"])';

/** Fokussiert das Steuerelement eines Sprungziels (`data-setting-anchor`); ohne genaue Entsprechung das erste Feld der Seite. */
export function focusAnchor(container: HTMLElement | null, anchor: string): boolean {
  if (!container) return false;
  const all = [...container.querySelectorAll<HTMLElement>("[data-setting-anchor]")];
  const el = all.find((e) => e.dataset.settingAnchor === anchor) ?? all.find((e) => (e.dataset.settingAnchor ?? "").startsWith(`${anchor}.`)) ?? all[0];
  if (!el) return false;
  const target = el.matches(FOCUSABLE) ? el : (el.querySelector<HTMLElement>(FOCUSABLE) ?? null);
  if (target) {
    target.focus();
    return true;
  }
  el.tabIndex = -1;
  el.focus();
  return true;
}

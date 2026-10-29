import type { I18n } from "./core";

/** Autonym plus completion of all English message values and attributes. */
export function languageOptions(catalog: I18n): { value: string; label: string }[] {
  return catalog.languages().map(value => {
    let name = value;
    try {
      name = new Intl.DisplayNames([value], { type: "language" }).of(value) ?? value;
      name = name.charAt(0).toUpperCase() + name.slice(1);
    } catch { /* Preserve resource locale when Intl has no display name. */ }
    const percent = catalog.completeness(value);
    return { value, label: percent < 100 ? `${name} (${percent}%)` : name };
  });
}

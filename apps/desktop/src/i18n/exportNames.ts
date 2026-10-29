// UI-121: export catalog names are stable command arguments, and may contain spaces/colons.
// Resolve their display names here so menus and export dialogs share the same translations.
import { canonKey } from "./canon";

type Lookup = (key: string, args?: Record<string, unknown>) => string | null;

export function exportNameKey(name: string): string {
  return `export-name-${name.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "")}`;
}

export function exportDisplayName(name: string, lookup: Lookup, separator = ": "): string | null {
  // Custom names are user data, including punctuation and colons; only their group and generated suffix are translated.
  const custom = /^(Custom Search|Custom Extraction):(.+)$/.exec(name);
  if (custom) {
    const group = lookup(canonKey("tab", custom[1]!));
    if (group === null) return null;
    let label = custom[2]!;
    const inlinks = label.endsWith(" Inlinks");
    if (inlinks) label = label.slice(0, -8);
    const generated = /^Extractor (\d+)$/.exec(label);
    if (generated) label = lookup("export-name-extractor", { n: generated[1] }) ?? label;
    else label = lookup(exportNameKey(label)) ?? label;
    if (inlinks) label = lookup("export-name-inlinks", { name: label }) ?? label;
    return `${group}${separator}${label}`;
  }
  const part = (value: string): string | null => {
    const direct = lookup(exportNameKey(value)) ?? lookup(canonKey("tab", value)) ?? lookup(canonKey("filter", value));
    if (direct !== null) return direct;
    for (const [pattern, key] of [
      [/^Images Over (\d+) KB$/, "export-name-images-over-kb"],
      [/^Images With Alt Text Over (\d+) Characters$/, "export-name-images-alt-over-characters"],
    ] as const) {
      const match = pattern.exec(value);
      if (match) return lookup(key, { n: match[1] });
    }
    for (const [suffix, key] of [[" (Bulk)", "export-name-bulk"], [" Inlinks", "export-name-inlinks"], [" Details", "export-name-details"], [" Triples", "export-name-triples"]] as const) {
      if (value.endsWith(suffix)) {
        const translated = part(value.slice(0, -suffix.length));
        if (translated !== null) return lookup(key, { name: translated });
      }
    }
    return null;
  };
  const parts = name.split(":").map(value => part(value.trim()));
  return parts.every((value): value is string => value !== null) ? parts.join(separator) : null;
}
